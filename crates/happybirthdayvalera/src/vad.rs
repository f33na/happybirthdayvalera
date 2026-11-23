//! Energy-based voice activity detection over RMS blocks.

use crate::shared::BEHAVIOR;

/// The RMS of one block of samples; `0.0` for an empty block.
#[must_use]
pub fn rms(block: &[f32]) -> f32 {
    if block.is_empty() {
        return 0.0;
    }
    let sum: f64 = block.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    #[allow(clippy::cast_precision_loss, reason = "a block holds a few hundred samples, far below 2^52")]
    let mean = sum / block.len() as f64;
    #[allow(clippy::cast_possible_truncation, reason = "the RMS of f32 samples fits in f32")]
    let out = mean.sqrt() as f32;
    out
}

/// VAD thresholds. [`Default`] takes them from `behavior.json`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VadConfig {
    /// Speech is a block with an RMS at or above this value.
    pub threshold_rms: f32,
    /// Speech starts after this long at or above the threshold.
    pub min_speech_ms: u32,
    /// After speech, this long below the threshold fires [`VadEvent::Silence`].
    pub silence_ms: u32,
    /// The recording stops at this length.
    pub max_duration_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold_rms: BEHAVIOR.vad_threshold_rms,
            min_speech_ms: BEHAVIOR.min_speech_ms,
            silence_ms: BEHAVIOR.silence_ms,
            max_duration_ms: BEHAVIOR.max_duration_ms,
        }
    }
}

/// What a block changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VadEvent {
    /// The RMS stayed at or above the threshold for `min_speech_ms`.
    SpeechStarted,
    /// After speech, the RMS stayed below the threshold for `silence_ms`.
    Silence,
    /// The recording reached `max_duration_ms`.
    MaxDuration,
}

/// The detector state for one recording.
#[derive(Clone, Debug)]
pub struct Vad {
    config: VadConfig,
    above_ms: f64,
    below_ms: f64,
    elapsed_ms: f64,
    speech: bool,
    done: bool,
}

impl Vad {
    /// A detector at the start of a recording.
    #[must_use]
    pub fn new(config: VadConfig) -> Self {
        Self { config, above_ms: 0.0, below_ms: 0.0, elapsed_ms: 0.0, speech: false, done: false }
    }

    /// True once speech has started in this recording.
    #[must_use]
    pub fn heard_speech(&self) -> bool {
        self.speech
    }

    /// Feeds one block. Returns at most one event; after `Silence` or `MaxDuration` it returns `None`.
    pub fn push(&mut self, rms: f32, block_ms: f64) -> Option<VadEvent> {
        if self.done {
            return None;
        }
        self.elapsed_ms += block_ms;
        if rms >= self.config.threshold_rms {
            self.above_ms += block_ms;
            self.below_ms = 0.0;
        } else {
            self.below_ms += block_ms;
            self.above_ms = 0.0;
        }

        if self.elapsed_ms >= f64::from(self.config.max_duration_ms) {
            self.done = true;
            return Some(VadEvent::MaxDuration);
        }
        if !self.speech {
            if self.above_ms >= f64::from(self.config.min_speech_ms) {
                self.speech = true;
                return Some(VadEvent::SpeechStarted);
            }
            return None;
        }
        if self.below_ms >= f64::from(self.config.silence_ms) {
            self.done = true;
            return Some(VadEvent::Silence);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK_MS: f64 = 20.0;
    const LOUD: f32 = 0.05;
    const QUIET: f32 = 0.002;

    fn cfg() -> VadConfig {
        VadConfig { threshold_rms: 0.012, min_speech_ms: 250, silence_ms: 1500, max_duration_ms: 30_000 }
    }

    /// Feeds `n` blocks and returns each event with the 1-based index of the block that fired it.
    fn feed(vad: &mut Vad, level: f32, n: usize, events: &mut Vec<(usize, VadEvent)>, offset: &mut usize) {
        for _ in 0..n {
            *offset += 1;
            if let Some(ev) = vad.push(level, BLOCK_MS) {
                events.push((*offset, ev));
            }
        }
    }

    #[test]
    fn rms_of_known_blocks() {
        assert!(rms(&[]).abs() < f32::EPSILON);
        assert!((rms(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6);
        let sine: Vec<f32> = (0..320).map(|i| (f32::from(u16::try_from(i).unwrap()) * 0.3).sin()).collect();
        assert!((rms(&sine) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);
    }

    #[test]
    fn silence_alone_never_stops() {
        let mut vad = Vad::new(cfg());
        let (mut events, mut at) = (Vec::new(), 0);
        feed(&mut vad, QUIET, 500, &mut events, &mut at);
        assert!(events.is_empty());
        assert!(!vad.heard_speech());
    }

    #[test]
    fn speech_needs_min_speech_ms_in_a_row() {
        let mut vad = Vad::new(cfg());
        let (mut events, mut at) = (Vec::new(), 0);
        // 240 ms loud, a quiet gap, then 260 ms loud: only the second burst counts.
        feed(&mut vad, LOUD, 12, &mut events, &mut at);
        feed(&mut vad, QUIET, 1, &mut events, &mut at);
        assert!(events.is_empty());
        feed(&mut vad, LOUD, 13, &mut events, &mut at);
        assert_eq!(events, [(26, VadEvent::SpeechStarted)]);
    }

    #[test]
    fn threshold_is_inclusive() {
        let mut vad = Vad::new(cfg());
        let (mut events, mut at) = (Vec::new(), 0);
        feed(&mut vad, 0.012, 13, &mut events, &mut at);
        assert_eq!(events, [(13, VadEvent::SpeechStarted)]);
    }

    #[test]
    fn silence_after_speech() {
        let mut vad = Vad::new(cfg());
        let (mut events, mut at) = (Vec::new(), 0);
        feed(&mut vad, LOUD, 20, &mut events, &mut at);
        // A short pause between words does not stop the recording.
        feed(&mut vad, QUIET, 70, &mut events, &mut at);
        feed(&mut vad, LOUD, 5, &mut events, &mut at);
        feed(&mut vad, QUIET, 80, &mut events, &mut at);
        assert_eq!(events, [(13, VadEvent::SpeechStarted), (170, VadEvent::Silence)]);
        // Nothing fires after the stop event.
        feed(&mut vad, LOUD, 50, &mut events, &mut at);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn max_duration_stops_even_during_speech() {
        let mut vad = Vad::new(VadConfig { max_duration_ms: 1000, ..cfg() });
        let (mut events, mut at) = (Vec::new(), 0);
        feed(&mut vad, LOUD, 100, &mut events, &mut at);
        assert_eq!(events, [(13, VadEvent::SpeechStarted), (50, VadEvent::MaxDuration)]);
    }

    #[test]
    fn defaults_come_from_behavior_json() {
        let d = VadConfig::default();
        assert!((d.threshold_rms - 0.012).abs() < f32::EPSILON);
        assert_eq!((d.min_speech_ms, d.silence_ms, d.max_duration_ms), (250, 1500, 30_000));
    }
}
