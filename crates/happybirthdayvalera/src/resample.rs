//! Streaming conversion of device audio to 16 kHz mono, and the split into ~20 ms blocks.
//!
//! Downsampling first runs a windowed-sinc low-pass so that content above 8 kHz does not alias into the
//! speech band, then interpolates linearly. The web `AudioWorklet` (`web/capture-worklet.js`) runs the same
//! algorithm.

use std::collections::VecDeque;
use std::f64::consts::PI;

/// The capture rate of the contract.
pub const TARGET_RATE: u32 = 16_000;

/// Samples in one level block: 20 ms at 16 kHz.
pub const BLOCK_SAMPLES: usize = 320;

/// A stateful resampler from `input_rate` to [`TARGET_RATE`].
#[derive(Clone, Debug)]
pub struct Resampler {
    step: f64,
    taps: Vec<f32>,
    history: VecDeque<f32>,
    prev: f32,
    pos: f64,
}

impl Resampler {
    /// A resampler for one stream.
    ///
    /// # Panics
    ///
    /// When `input_rate` is zero.
    #[must_use]
    pub fn new(input_rate: u32) -> Self {
        assert!(input_rate > 0, "the input rate must be positive");
        let step = f64::from(input_rate) / f64::from(TARGET_RATE);
        let taps = if step > 1.0 { low_pass(step) } else { vec![1.0] };
        let history = VecDeque::from(vec![0.0; taps.len()]);
        Self { step, taps, history, prev: 0.0, pos: 0.0 }
    }

    /// Converts interleaved frames with `channels` channels and appends 16 kHz mono samples to `out`.
    pub fn process_interleaved(&mut self, input: &[f32], channels: usize, out: &mut Vec<f32>) {
        let channels = channels.max(1);
        #[allow(clippy::cast_precision_loss, reason = "a channel count is tiny")]
        let scale = 1.0 / channels as f32;
        for frame in input.chunks_exact(channels) {
            let mono = frame.iter().sum::<f32>() * scale;
            self.push(mono, out);
        }
    }

    fn push(&mut self, sample: f32, out: &mut Vec<f32>) {
        self.history.pop_front();
        self.history.push_back(sample);
        let filtered: f32 = self.history.iter().zip(&self.taps).map(|(x, h)| x * h).sum();
        // Emit every output position that falls in (previous input sample, this input sample].
        while self.pos <= 1.0 {
            #[allow(clippy::cast_possible_truncation, reason = "pos is in [0, 1]")]
            let frac = self.pos as f32;
            out.push(self.prev + (filtered - self.prev) * frac);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = filtered;
    }
}

/// Blackman-windowed sinc taps with the cutoff at 90 % of the output Nyquist rate, normalized to unity gain.
fn low_pass(step: f64) -> Vec<f32> {
    let cutoff = 0.45 / step; // cycles per input sample
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "step is a small positive ratio")]
    let half = (8.0 * step).ceil() as usize;
    let n = 2 * half + 1;
    #[allow(clippy::cast_precision_loss, reason = "a few dozen taps")]
    let taps: Vec<f64> = (0..n)
        .map(|i| {
            let m = i as f64 - half as f64;
            let sinc = if m == 0.0 { 2.0 * cutoff } else { (2.0 * PI * cutoff * m).sin() / (PI * m) };
            let w = 2.0 * PI * i as f64 / (n - 1) as f64;
            let window = 0.42 - 0.5 * w.cos() + 0.08 * (2.0 * w).cos();
            sinc * window
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    #[allow(clippy::cast_possible_truncation, reason = "filter taps fit in f32")]
    taps.into_iter().map(|h| (h / sum) as f32).collect()
}

/// Collects samples into blocks of [`BLOCK_SAMPLES`].
#[derive(Clone, Debug, Default)]
pub struct Blocker {
    pending: Vec<f32>,
}

impl Blocker {
    /// Appends samples and returns every full block.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.pending.extend_from_slice(samples);
        let full = self.pending.len() / BLOCK_SAMPLES;
        if full == 0 {
            return Vec::new();
        }
        let rest = self.pending.split_off(full * BLOCK_SAMPLES);
        let ready = std::mem::replace(&mut self.pending, rest);
        ready.chunks_exact(BLOCK_SAMPLES).map(<[f32]>::to_vec).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vad::rms;

    fn sine(rate: u32, freq: f64, seconds: f64, channels: usize) -> Vec<f32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "test sizes")]
        let n = (f64::from(rate) * seconds) as usize;
        let mut out = Vec::with_capacity(n * channels);
        for i in 0..n {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, reason = "test signal")]
            let v = (0.5 * (2.0 * PI * freq * i as f64 / f64::from(rate)).sin()) as f32;
            out.extend(std::iter::repeat_n(v, channels));
        }
        out
    }

    fn resample(rate: u32, input: &[f32], channels: usize) -> Vec<f32> {
        let mut r = Resampler::new(rate);
        let mut out = Vec::new();
        // Odd chunk sizes check that the state carries over between callbacks.
        for chunk in input.chunks(channels * 37) {
            r.process_interleaved(chunk, channels, &mut out);
        }
        out
    }

    /// Counts sign changes to estimate the frequency of a clean tone.
    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
    }

    #[test]
    fn keeps_the_duration_and_the_tone() {
        for rate in [48_000, 44_100, 22_050, 16_000, 8_000] {
            let out = resample(rate, &sine(rate, 440.0, 1.0, 2), 2);
            let len = i64::try_from(out.len()).unwrap();
            assert!((len - 16_000).abs() <= 2, "{rate} Hz gave {len} samples");
            let body = &out[1600..14_400]; // skip the filter warm-up at both ends
            let rms = rms(body);
            assert!((rms - 0.5 * std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01, "{rate} Hz rms {rms}");
            let crossings = zero_crossings(body);
            assert!((700..=710).contains(&crossings), "{rate} Hz: {crossings} crossings in 0.8 s of 440 Hz");
        }
    }

    #[test]
    fn suppresses_content_above_the_new_nyquist_rate() {
        let out = resample(48_000, &sine(48_000, 12_000.0, 1.0, 1), 1);
        let level = rms(&out[1600..14_400]);
        assert!(level < 0.01, "a 12 kHz tone leaked through at rms {level}");
    }

    #[test]
    fn target_rate_matches_behavior_json() {
        let behavior: serde_json::Value = serde_json::from_str(crate::shared::BEHAVIOR_JSON).unwrap();
        assert_eq!(behavior["sampleRate"], u64::from(TARGET_RATE));
    }

    #[test]
    fn blocker_returns_full_blocks_and_keeps_the_rest() {
        let mut b = Blocker::default();
        assert!(b.push(&[0.0; 300]).is_empty());
        let blocks = b.push(&[1.0; 400]);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().all(|blk| blk.len() == BLOCK_SAMPLES));
        assert_eq!(b.pending.len(), 60);
    }
}
