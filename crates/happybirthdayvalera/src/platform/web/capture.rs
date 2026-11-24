//! Microphone capture in the browser: `getUserMedia` → `MediaStreamSource` → the `hbv-capture` `AudioWorklet`.
//!
//! The worklet code is compiled into the crate and loaded from a Blob URL, so the host serves no extra file.
//! The worklet resamples to 16 kHz mono and posts 320-sample blocks; the main thread forwards them to the
//! listener. Dropping [`Capture`] stops the tracks and closes the audio context at once.

use futures_channel::mpsc::UnboundedSender;
use js_sys::{Array, Float32Array, Object, Reflect};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioWorkletNode, AudioWorkletNodeOptions, Blob, BlobPropertyBag, MediaStream,
    MediaStreamConstraints, MediaStreamTrack, MessageEvent, Url,
};

use crate::machine::ErrorCode;

const WORKLET_JS: &str = include_str!("capture-worklet.js");

/// A running capture. Drop it to release the microphone.
pub(crate) struct Capture {
    stream: MediaStream,
    context: AudioContext,
    node: AudioWorkletNode,
    _on_block: Closure<dyn FnMut(MessageEvent)>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Ok(port) = self.node.port() {
            port.set_onmessage(None);
        }
        stop_tracks(&self.stream);
        // Disconnecting a node that is already disconnected throws; there is nothing to undo then.
        let _ = self.node.disconnect();
        match self.context.close() {
            Ok(promise) => drop(promise),
            Err(e) => tracing::warn!("cannot close the audio context: {e:?}"),
        }
    }
}

fn stop_tracks(stream: &MediaStream) {
    for track in stream.get_tracks().iter() {
        if let Ok(track) = track.dyn_into::<MediaStreamTrack>() {
            track.stop();
        }
    }
}

/// The `DOMException` name of a rejected `getUserMedia`, mapped to the contract codes.
fn map_media_error(e: &JsValue) -> ErrorCode {
    let name = Reflect::get(e, &JsValue::from_str("name")).ok().and_then(|n| n.as_string()).unwrap_or_default();
    tracing::warn!("getUserMedia failed: {name}: {e:?}");
    match name.as_str() {
        "NotAllowedError" | "SecurityError" | "PermissionDeniedError" => ErrorCode::MicDenied,
        "TypeError" => ErrorCode::Unsupported,
        // NotFoundError, NotReadableError, OverconstrainedError, AbortError and unknown names.
        _ => ErrorCode::MicUnavailable,
    }
}

fn set(target: &Object, key: &str, value: &JsValue) {
    // `Reflect.set` on a plain new object cannot fail.
    let _ = Reflect::set(target, &JsValue::from_str(key), value);
}

pub(crate) async fn open_capture(sink: UnboundedSender<Vec<f32>>) -> Result<Capture, ErrorCode> {
    let window = web_sys::window().ok_or(ErrorCode::Unsupported)?;
    // `mediaDevices` is missing outside a secure context (plain http on a non-local host).
    let devices = window.navigator().media_devices().map_err(|_| ErrorCode::Unsupported)?;
    if JsValue::from(devices.clone()).is_undefined() {
        return Err(ErrorCode::Unsupported);
    }

    let audio = Object::new();
    set(&audio, "channelCount", &JsValue::from(1));
    set(&audio, "echoCancellation", &JsValue::TRUE);
    set(&audio, "noiseSuppression", &JsValue::TRUE);
    set(&audio, "autoGainControl", &JsValue::TRUE);
    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&audio);
    let promise = devices.get_user_media_with_constraints(&constraints).map_err(|e| map_media_error(&e))?;
    let stream: MediaStream = JsFuture::from(promise)
        .await
        .map_err(|e| map_media_error(&e))?
        .dyn_into()
        .map_err(|_| ErrorCode::Unsupported)?;

    match connect(&stream, sink).await {
        Ok(capture) => Ok(capture),
        Err(e) => {
            tracing::warn!("cannot start the audio graph: {e:?}");
            stop_tracks(&stream);
            Err(ErrorCode::Unsupported)
        }
    }
}

async fn connect(stream: &MediaStream, sink: UnboundedSender<Vec<f32>>) -> Result<Capture, JsValue> {
    let context = AudioContext::new()?;
    let result = build_graph(&context, stream, sink).await;
    if result.is_err() {
        // Free the context of a failed start; a close failure changes nothing for the caller.
        let _ = context.close();
    }
    let (node, on_block) = result?;
    // `Clone::clone` copies the JS handle. The inherent `MediaStream::clone` would call the JS `clone()` and
    // create new tracks, so `Drop` would stop those while the getUserMedia track, and the browser's mic
    // indicator, stayed live. clippy.toml bans the inherent method in this crate.
    Ok(Capture { stream: Clone::clone(stream), context, node, _on_block: on_block })
}

async fn build_graph(
    context: &AudioContext,
    stream: &MediaStream,
    sink: UnboundedSender<Vec<f32>>,
) -> Result<(AudioWorkletNode, Closure<dyn FnMut(MessageEvent)>), JsValue> {
    let parts = Array::of1(&JsValue::from_str(WORKLET_JS));
    let options = BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let added = context.audio_worklet().and_then(|worklet| worklet.add_module(&url));
    let added = match added {
        Ok(promise) => JsFuture::from(promise).await.map(drop),
        Err(e) => Err(e),
    };
    // The module is parsed once `addModule` settles, so the URL is no longer needed either way.
    let _ = Url::revoke_object_url(&url);
    added?;

    let node_options = AudioWorkletNodeOptions::new();
    node_options.set_number_of_inputs(1);
    node_options.set_number_of_outputs(1);
    node_options.set_output_channel_count(&Array::of1(&JsValue::from(1)));
    let node = AudioWorkletNode::new_with_options(context, "hbv-capture", &node_options)?;

    let on_block = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Ok(block) = event.data().dyn_into::<Float32Array>() {
            // A closed channel means the listener is gone; the drop of `Capture` stops the graph.
            let _ = sink.unbounded_send(block.to_vec());
        }
    });
    node.port()?.set_onmessage(Some(on_block.as_ref().unchecked_ref()));

    let source = context.create_media_stream_source(stream)?;
    source.connect_with_audio_node(&node)?;
    // A muted path to the destination keeps the worklet pulled by the graph in every browser.
    let mute = context.create_gain()?;
    mute.gain().set_value(0.0);
    node.connect_with_audio_node(&mute)?;
    mute.connect_with_audio_node(&context.destination())?;
    // The click that started the capture allows audio; resume in case the context began suspended.
    JsFuture::from(context.resume()?).await?;
    Ok((node, on_block))
}
