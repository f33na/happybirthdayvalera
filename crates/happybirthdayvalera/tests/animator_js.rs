//! The in-page animator (`src/animator.js`) draws the waveform with its own copy of the contract formula. This
//! test runs its path and smoothing functions under Node and checks them against the shared fixture, so the
//! JavaScript twin cannot drift from `src/wave.rs`. Node is a documented dependency of the repository.

use std::process::Command;

use happybirthdayvalera::wave::{smooth_level, tokens};

const ANIMATOR: &str = include_str!("../src/animator.js");
const FIXTURE: &str = include_str!("../../../shared/wave-fixture.json");

fn node(script: &str) -> String {
    let out = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .output()
        .unwrap_or_else(|e| panic!("cannot run `node` (install Node 20+ to run this test): {e}"));
    assert!(out.status.success(), "node failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("node prints UTF-8")
}

#[test]
fn js_paths_match_the_shared_fixture() {
    let tok = serde_json::to_string(tokens()).unwrap();
    let script = format!(
        "{ANIMATOR}\nconst tok = {tok};\nconst fixture = {FIXTURE};\n\
         const out = fixture.cases.map((c) => hbvStrandPath(tok, c.strand, c.t, hbvAmplitude(tok, c.state, c.level, false, false)));\n\
         console.log(JSON.stringify(out));"
    );
    let got: Vec<String> = serde_json::from_str(node(&script).trim()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(got.len(), cases.len());
    assert!(!got.is_empty());
    for (path, case) in got.iter().zip(cases) {
        assert_eq!(path, case["d"].as_str().unwrap(), "strand {}", case["strand"]);
    }
}

#[test]
fn js_smoothing_and_reduced_motion_match_rust() {
    let tok_json = serde_json::to_string(tokens()).unwrap();
    let script = format!(
        "{ANIMATOR}\nconst tok = {tok_json};\n\
         let l = 0; const seq = [];\n\
         for (const rms of [0.01, 0.2, 0.2, 0.0, 0.0, 0.05]) {{ l = hbvSmooth(tok, l, rms); seq.push(l); }}\n\
         console.log(JSON.stringify({{ seq, reduced: hbvAmplitude(tok, 'listening', 0.7, false, true), rest: hbvAmplitude(tok, 'idle', 0.7, false, false), party: hbvAmplitude(tok, 'idle', 0, true, false) }}));"
    );
    let got: serde_json::Value = serde_json::from_str(node(&script).trim()).unwrap();
    let tok = tokens();
    let mut level = 0.0;
    for (i, rms) in [0.01, 0.2, 0.2, 0.0, 0.0, 0.05].into_iter().enumerate() {
        level = smooth_level(tok, level, rms);
        let js = got["seq"][i].as_f64().unwrap();
        assert!((js - level).abs() < 1e-12, "step {i}: js {js}, rust {level}");
    }
    assert!((got["reduced"].as_f64().unwrap() - tok.reduced_motion_amp).abs() < 1e-12);
    assert!((got["rest"].as_f64().unwrap() - tok.rest_amp).abs() < 1e-12);
    let party = happybirthdayvalera::wave::amplitude(tok, happybirthdayvalera::State::Idle, 0.0, true, false);
    assert!((got["party"].as_f64().unwrap() - party).abs() < 1e-12);
}
