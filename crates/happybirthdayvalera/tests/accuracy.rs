//! Recognition accuracy over `fixtures/`: whisper.cpp transcribes each phrase with the language forced, and
//! the text must contain every key word. It checks the default model of `behavior.json` (`tiny`); the first
//! run downloads `ggml-tiny.bin` (78 MB) into the default model store, from `HBV_MODEL_BASE_URL` when that is
//! set. Run with `cargo test -p happybirthdayvalera --features desktop -- --ignored`.
#![cfg(all(feature = "desktop", not(target_arch = "wasm32")))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_executor::block_on;
use happybirthdayvalera::easter::is_birthday_phrase;
use happybirthdayvalera::{Lang, ModelSize, ModelStore, SpeechEngine as _, WhisperEngine};
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected {
    keywords: Vec<String>,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Reads a 16 kHz mono 16-bit PCM WAV. It walks the RIFF chunks, so extra chunks such as the `FLLR` padding
/// that `afconvert` writes before `data` are skipped.
fn read_wav(path: &Path) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE", "{}: not a WAV", path.display());
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let mut at = 12;
    let mut format = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32_at(at + 4) as usize;
        let body = at + 8;
        let end = (body + len).min(bytes.len());
        match id {
            b"fmt " => format = Some((u16_at(body), u16_at(body + 2), u32_at(body + 4), u16_at(body + 14))),
            b"data" => {
                assert_eq!(format, Some((1, 1, 16_000, 16)), "{}: want PCM mono 16 kHz 16-bit", path.display());
                return bytes[body..end]
                    .chunks_exact(2)
                    .map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32_768.0)
                    .collect();
            }
            _ => {}
        }
        at = body + len + (len & 1); // chunks are padded to an even size
    }
    panic!("{}: no data chunk", path.display());
}

#[test]
fn wav_reader_skips_extra_chunks() {
    let pcm = read_wav(&fixtures().join("en.wav"));
    assert!(pcm.len() > 16_000, "en.wav holds more than one second");
    assert!(pcm.iter().any(|s| s.abs() > 0.05), "en.wav is not silent");
}

#[test]
#[ignore = "downloads the default model on the first run and takes about a minute on a CPU"]
fn the_default_model_hears_every_fixture() {
    let raw = std::fs::read_to_string(fixtures().join("expected.json")).unwrap();
    let expected: BTreeMap<String, serde_json::Value> = serde_json::from_str(&raw).unwrap();
    let store = match std::env::var("HBV_MODEL_BASE_URL") {
        Ok(base) if !base.is_empty() => ModelStore::default().with_base_url(base),
        _ => ModelStore::default(),
    };
    let engine = WhisperEngine::with_store(store);
    let model = ModelSize::default();
    block_on(engine.load(model, &|_| {})).unwrap_or_else(|e| panic!("the {model} model loads: {e}"));

    let mut failures = Vec::new();
    for (code, value) in expected.into_iter().filter(|(k, _)| !k.starts_with('$')) {
        let lang: Lang = code.parse().unwrap();
        let want: Expected = serde_json::from_value(value).unwrap();
        let pcm = read_wav(&fixtures().join(format!("{code}.wav")));
        let text = block_on(engine.transcribe(pcm, lang)).unwrap_or_else(|e| panic!("{code}: {e}"));
        println!("{code}: {text}");
        let folded = text.to_lowercase().replace('ё', "е");
        for keyword in &want.keywords {
            if !folded.contains(keyword.as_str()) {
                failures.push(format!("{code}: `{keyword}` missing in `{text}`"));
            }
        }
        if !is_birthday_phrase(&text) {
            failures.push(format!("{code}: `{text}` does not trigger the easter egg"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
