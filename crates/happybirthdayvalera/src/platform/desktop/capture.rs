//! Microphone capture with cpal on a dedicated thread.
//!
//! A cpal `Stream` is not `Send` on every host, so one thread builds it, owns it and drops it. The audio
//! callback converts to 16 kHz mono, cuts 20 ms blocks and sends them over an unbounded channel. Dropping
//! [`Capture`] wakes the thread, which drops the stream and exits; the drop waits for that, so the
//! microphone is released when the drop returns.
//!
//! Before the stream opens, macOS is asked for access (see `permission`). After it opens, the first samples
//! go through [`ZeroProbe`]: a stream of exact zeros is a denied or dead microphone, not a quiet room.

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as _, SampleFormat, SizedSample};
use futures_channel::mpsc::UnboundedSender;
use futures_channel::oneshot;

use super::permission::{SILENT_INPUT, ensure_access};
use crate::machine::ErrorCode;
use crate::resample::{Blocker, Resampler, TARGET_RATE};

/// About 700 ms of 16 kHz audio. A working microphone has some noise in every block, so it is never
/// bit-exact zero for this long.
const PROBE_SAMPLES: usize = TARGET_RATE as usize * 7 / 10;

/// How long to wait for the probe. No audio at all in this time means the device does not deliver.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// What the first samples of a stream say about the microphone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Probe {
    /// A sample that is not exactly zero arrived.
    Live,
    /// `PROBE_SAMPLES` exact zeros arrived and nothing else.
    Dead,
}

/// Watches the first blocks of a stream. It decides at the first non-zero sample, or after
/// `PROBE_SAMPLES` zeros; until then `push` returns `None`.
#[derive(Debug, Default)]
pub(crate) struct ZeroProbe {
    zeros: usize,
}

impl ZeroProbe {
    pub(crate) fn push(&mut self, block: &[f32]) -> Option<Probe> {
        // `-0.0 == 0.0`, so both zeros count as digital silence; every other value is real signal.
        if block.iter().any(|&s| s != 0.0) {
            return Some(Probe::Live);
        }
        self.zeros += block.len();
        (self.zeros >= PROBE_SAMPLES).then_some(Probe::Dead)
    }
}

/// A running capture. Drop it to release the microphone.
pub(crate) struct Capture {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Closing the channel is the stop signal.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            join_logged(thread);
        }
    }
}

pub(crate) async fn open_capture(sink: UnboundedSender<Vec<f32>>) -> Result<Capture, ErrorCode> {
    ensure_access().await?;
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(), ErrorCode>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("hbv-capture".into())
        .spawn(move || run(sink, ready_tx, &stop_rx))
        .map_err(|e| {
            tracing::warn!("cannot start the capture thread: {e}");
            ErrorCode::MicUnavailable
        })?;
    // Until the thread answers, a drop of this future (the listener unmounted) must not block the UI thread
    // on a join while the OS may still show its permission prompt. The locals then drop on their own: the
    // closed `stop_tx` and the failed `ready` send make the thread release the stream and exit by itself.
    match ready_rx.await {
        Ok(Ok(())) => Ok(Capture { stop: Some(stop_tx), thread: Some(thread) }),
        Ok(Err(code)) => {
            join_logged(thread);
            Err(code)
        }
        Err(oneshot::Canceled) => {
            // The thread ended without an answer, so it panicked.
            join_logged(thread);
            Err(ErrorCode::MicUnavailable)
        }
    }
}

fn join_logged(thread: JoinHandle<()>) {
    if thread.join().is_err() {
        tracing::warn!("the capture thread panicked");
    }
}

fn run(sink: UnboundedSender<Vec<f32>>, ready: oneshot::Sender<Result<(), ErrorCode>>, stop: &mpsc::Receiver<()>) {
    match start_stream(sink) {
        Ok((stream, probe)) => {
            let verdict = match probe.recv_timeout(PROBE_TIMEOUT) {
                Ok(Probe::Live) => Ok(()),
                Ok(Probe::Dead) => {
                    tracing::warn!("the input delivers only exact zeros; treating it as {SILENT_INPUT:?}");
                    Err(SILENT_INPUT)
                }
                Err(e) => {
                    tracing::warn!("no audio from the input stream within {PROBE_TIMEOUT:?}: {e}");
                    Err(ErrorCode::MicUnavailable)
                }
            };
            let failed = verdict.is_err();
            if ready.send(verdict).is_err() || failed {
                return; // the listener stopped waiting or the input is unusable; dropping `stream` releases it
            }
            // `recv` returns when the sender drops; no message is ever sent.
            let _ = stop.recv();
            drop(stream);
        }
        Err(code) => {
            // A closed channel means the listener gave up waiting; nobody needs the error then.
            let _ = ready.send(Err(code));
        }
    }
}

fn start_stream(sink: UnboundedSender<Vec<f32>>) -> Result<(cpal::Stream, mpsc::Receiver<Probe>), ErrorCode> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| {
        tracing::warn!("no default input device on host {:?}", host.id());
        ErrorCode::MicUnavailable
    })?;
    let supported = device.default_input_config().map_err(|e| map_error("read the input config", &e))?;
    let config = supported.config();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let (probe_tx, probe_rx) = mpsc::channel::<Probe>();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::I16 => build::<i16>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::U16 => build::<u16>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::I32 => build::<i32>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::I8 => build::<i8>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::U8 => build::<u8>(&device, config, channels, rate, sink, probe_tx),
        SampleFormat::F64 => build::<f64>(&device, config, channels, rate, sink, probe_tx),
        other => {
            tracing::warn!("unsupported input sample format {other:?}");
            return Err(ErrorCode::Unsupported);
        }
    }?;
    stream.play().map_err(|e| map_error("start the input stream", &e))?;
    Ok((stream, probe_rx))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    rate: u32,
    sink: UnboundedSender<Vec<f32>>,
    probe_tx: mpsc::Sender<Probe>,
) -> Result<cpal::Stream, ErrorCode>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let mut resampler = Resampler::new(rate);
    let mut blocker = Blocker::default();
    let mut frames: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    // Dropped after the first verdict, so the callback stops checking.
    let mut probe = Some((ZeroProbe::default(), probe_tx));
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _info: &cpal::InputCallbackInfo| {
                frames.clear();
                frames.extend(data.iter().map(|&s| f32::from_sample(s)));
                mono.clear();
                resampler.process_interleaved(&frames, channels, &mut mono);
                for block in blocker.push(&mono) {
                    if let Some((watch, tx)) = probe.as_mut()
                        && let Some(verdict) = watch.push(&block)
                    {
                        // A closed channel means the capture thread stopped waiting.
                        let _ = tx.send(verdict);
                        probe = None;
                    }
                    if sink.unbounded_send(block).is_err() {
                        return; // the listener is gone; the stream stops when the thread drops it
                    }
                }
            },
            |e| tracing::warn!("input stream error: {e}"),
            None,
        )
        .map_err(|e| map_error("open the input stream", &e))
}

fn map_error(action: &str, e: &cpal::Error) -> ErrorCode {
    tracing::warn!("cannot {action}: {e}");
    match e.kind() {
        cpal::ErrorKind::PermissionDenied => ErrorCode::MicDenied,
        cpal::ErrorKind::UnsupportedConfig | cpal::ErrorKind::UnsupportedOperation => ErrorCode::Unsupported,
        // `ErrorKind` is non-exhaustive; every other kind means the device cannot be used right now.
        _ => ErrorCode::MicUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resample::BLOCK_SAMPLES;

    /// Feeds 20 ms blocks of `signal` until the probe decides; returns the verdict and the samples used.
    fn run(mut signal: impl FnMut(usize) -> f32, max_samples: usize) -> (Option<Probe>, usize) {
        let mut probe = ZeroProbe::default();
        let mut used = 0;
        while used < max_samples {
            let block: Vec<f32> = (used..used + BLOCK_SAMPLES).map(&mut signal).collect();
            used += BLOCK_SAMPLES;
            if let Some(v) = probe.push(&block) {
                return (Some(v), used);
            }
        }
        (None, used)
    }

    #[test]
    fn exact_zeros_are_a_dead_input_after_700_ms() {
        let (verdict, used) = run(|_| 0.0, TARGET_RATE as usize);
        assert_eq!(verdict, Some(Probe::Dead));
        assert!((PROBE_SAMPLES..PROBE_SAMPLES + BLOCK_SAMPLES).contains(&used), "{used}");
        // Negative zero is digital silence too.
        assert_eq!(run(|_| -0.0, TARGET_RATE as usize).0, Some(Probe::Dead));
    }

    #[test]
    fn zeros_shorter_than_the_window_do_not_decide() {
        assert_eq!(run(|_| 0.0, PROBE_SAMPLES - BLOCK_SAMPLES).0, None);
    }

    #[test]
    fn low_noise_is_a_live_input_at_once() {
        // A quiet room through a working microphone: tiny but never exactly zero.
        let (verdict, used) = run(|i| if i % 7 == 0 { 3.0e-5 } else { -1.0e-5 }, TARGET_RATE as usize);
        assert_eq!(verdict, Some(Probe::Live));
        assert_eq!(used, BLOCK_SAMPLES);
    }

    #[test]
    fn speech_after_a_zero_gap_is_a_live_input() {
        // Some drivers start with a few blocks of zeros before real audio.
        #[allow(clippy::cast_precision_loss, reason = "sample indices stay far below 2^24")]
        let speech = |i: usize| if i < 3 * BLOCK_SAMPLES { 0.0 } else { 0.3 * (i as f32 * 0.07).sin() };
        let (verdict, used) = run(speech, TARGET_RATE as usize);
        assert_eq!(verdict, Some(Probe::Live));
        assert_eq!(used, 4 * BLOCK_SAMPLES);
    }
}
