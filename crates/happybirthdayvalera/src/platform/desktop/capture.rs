//! Microphone capture with cpal on a dedicated thread.
//!
//! A cpal `Stream` is not `Send` on every host, so one thread builds it, owns it and drops it. The audio
//! callback converts to 16 kHz mono, cuts 20 ms blocks and sends them over an unbounded channel. Dropping
//! [`Capture`] wakes the thread, which drops the stream and exits; the drop waits for that, so the
//! microphone is released when the drop returns.

use std::sync::mpsc;
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as _, SampleFormat, SizedSample};
use futures_channel::mpsc::UnboundedSender;
use futures_channel::oneshot;

use crate::machine::ErrorCode;
use crate::resample::{Blocker, Resampler};

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
        Ok(stream) => {
            if ready.send(Ok(())).is_err() {
                return; // the listener stopped waiting; dropping `stream` releases the microphone
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

fn start_stream(sink: UnboundedSender<Vec<f32>>) -> Result<cpal::Stream, ErrorCode> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| {
        tracing::warn!("no default input device on host {:?}", host.id());
        ErrorCode::MicUnavailable
    })?;
    let supported = device.default_input_config().map_err(|e| map_error("read the input config", &e))?;
    let config = supported.config();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, channels, rate, sink),
        SampleFormat::I16 => build::<i16>(&device, config, channels, rate, sink),
        SampleFormat::U16 => build::<u16>(&device, config, channels, rate, sink),
        SampleFormat::I32 => build::<i32>(&device, config, channels, rate, sink),
        SampleFormat::I8 => build::<i8>(&device, config, channels, rate, sink),
        SampleFormat::U8 => build::<u8>(&device, config, channels, rate, sink),
        SampleFormat::F64 => build::<f64>(&device, config, channels, rate, sink),
        other => {
            tracing::warn!("unsupported input sample format {other:?}");
            return Err(ErrorCode::Unsupported);
        }
    }?;
    stream.play().map_err(|e| map_error("start the input stream", &e))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    rate: u32,
    sink: UnboundedSender<Vec<f32>>,
) -> Result<cpal::Stream, ErrorCode>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let mut resampler = Resampler::new(rate);
    let mut blocker = Blocker::default();
    let mut frames: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _info: &cpal::InputCallbackInfo| {
                frames.clear();
                frames.extend(data.iter().map(|&s| f32::from_sample(s)));
                mono.clear();
                resampler.process_interleaved(&frames, channels, &mut mono);
                for block in blocker.push(&mono) {
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
