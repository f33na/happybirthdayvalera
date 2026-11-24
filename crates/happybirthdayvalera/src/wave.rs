//! The waveform math of the contract: level smoothing, amplitude and the SVG path of one strand.

use std::f64::consts::PI;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::machine::State;
use crate::shared::TOKENS;

/// One strand of the waveform.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strand {
    /// Cycles across the width.
    pub freq: f64,
    /// Radians per second.
    pub speed: f64,
    /// Radians at `t = 0`.
    pub phase: f64,
    /// Relative amplitude.
    pub amp: f64,
    /// Stroke opacity.
    pub opacity: f64,
}

/// `design-tokens.json` → `wave`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs, reason = "the fields mirror the token names that CONTRACT.md documents")]
pub struct WaveTokens {
    pub view_box_width: f64,
    pub view_box_height: f64,
    pub points: u32,
    pub stroke_width: f64,
    pub stroke_pad: f64,
    pub idle_amp: f64,
    pub rest_amp: f64,
    pub reduced_motion_amp: f64,
    pub level_gain: f64,
    pub attack: f64,
    pub release: f64,
    pub strands: Vec<Strand>,
}

/// The tokens compiled into the crate.
#[must_use]
pub fn tokens() -> &'static WaveTokens {
    &TOKENS.wave
}

/// One smoothing step: moves `level` toward `clamp(rms · levelGain, 0, 1)` with the attack or release rate.
#[must_use]
pub fn smooth_level(tok: &WaveTokens, level: f64, rms: f64) -> f64 {
    let target = (rms * tok.level_gain).clamp(0.0, 1.0);
    let rate = if target > level { tok.attack } else { tok.release };
    level + (target - level) * rate
}

/// The level the waveform uses while the easter egg plays.
pub const CELEBRATE_LEVEL: f64 = 0.6;

/// The amplitude `A` for a state and a smoothed level. While `celebrating`, the waveform moves as if the level
/// were [`CELEBRATE_LEVEL`]. Reduced motion wins over both.
#[must_use]
pub fn amplitude(tok: &WaveTokens, state: State, level: f64, celebrating: bool, reduced_motion: bool) -> f64 {
    if reduced_motion {
        tok.reduced_motion_amp
    } else if celebrating {
        tok.idle_amp + (1.0 - tok.idle_amp) * CELEBRATE_LEVEL
    } else if state == State::Listening {
        tok.idle_amp + (1.0 - tok.idle_amp) * level
    } else {
        tok.rest_amp
    }
}

/// The `d` attribute of strand `i` at time `t` (seconds since mount) and amplitude `amp`.
///
/// # Panics
///
/// When `i` is not a strand index of `tok`.
#[must_use]
#[allow(clippy::many_single_char_names, reason = "the names follow the formula in CONTRACT.md")]
pub fn strand_path(tok: &WaveTokens, i: usize, t: f64, amp: f64) -> String {
    let s = &tok.strands[i];
    let (w, h) = (tok.view_box_width, tok.view_box_height);
    let last = f64::from(tok.points.max(2) - 1);
    let mut d = String::with_capacity(tok.points as usize * 14);
    for k in 0..tok.points.max(2) {
        let x = f64::from(k) / last;
        let env = (PI * x).sin().powi(2);
        let y = h / 2.0 + (h / 2.0 - tok.stroke_pad) * env * s.amp * amp * (2.0 * PI * s.freq * x + s.phase + s.speed * t).sin();
        if k > 0 {
            d.push(' ');
        }
        d.push(if k == 0 { 'M' } else { 'L' });
        push_coord(&mut d, x * w);
        d.push(' ');
        push_coord(&mut d, y);
    }
    d
}

/// Writes `round(v · 100) / 100` with exactly two decimals, and `-0.00` as `0.00`.
fn push_coord(out: &mut String, v: f64) {
    let rounded = (v * 100.0).round() / 100.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    // Writing to a String cannot fail.
    let _ = write!(out, "{rounded:.2}");
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        strand: usize,
        t: f64,
        level: f64,
        state: String,
        d: String,
    }

    #[test]
    fn matches_every_case_of_the_shared_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/wave-fixture.json");
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let fixture: Fixture = serde_json::from_str(&raw).expect("wave-fixture.json parses");
        assert!(!fixture.cases.is_empty(), "the fixture has no cases");
        let tok = tokens();
        for case in &fixture.cases {
            let state: State = case.state.parse().unwrap();
            let amp = amplitude(tok, state, case.level, false, false);
            assert_eq!(strand_path(tok, case.strand, case.t, amp), case.d, "strand {}", case.strand);
        }
    }

    #[test]
    fn rest_and_reduced_motion_amplitudes() {
        let tok = tokens();
        assert!((amplitude(tok, State::Idle, 1.0, false, false) - tok.rest_amp).abs() < 1e-12);
        assert!((amplitude(tok, State::Listening, 0.0, false, false) - tok.idle_amp).abs() < 1e-12);
        assert!((amplitude(tok, State::Listening, 1.0, false, false) - 1.0).abs() < 1e-12);
        assert!((amplitude(tok, State::Listening, 0.3, false, true) - tok.reduced_motion_amp).abs() < 1e-12);
        let party = tok.idle_amp + (1.0 - tok.idle_amp) * 0.6;
        assert!((amplitude(tok, State::Idle, 0.0, true, false) - party).abs() < 1e-12);
        assert!((amplitude(tok, State::Idle, 0.0, true, true) - tok.reduced_motion_amp).abs() < 1e-12);
    }

    #[test]
    fn smoothing_uses_attack_up_and_release_down() {
        let tok = tokens();
        let up = smooth_level(tok, 0.0, 1.0);
        assert!((up - tok.attack).abs() < 1e-12, "the target clamps to 1, one attack step");
        let down = smooth_level(tok, 1.0, 0.0);
        assert!((down - (1.0 - tok.release)).abs() < 1e-12);
    }

    #[test]
    fn flat_path_has_no_negative_zero() {
        let d = strand_path(tokens(), 0, 0.0, 0.0);
        assert!(!d.contains("-0.00"));
        assert!(d.starts_with("M0.00 14.00 L"));
        assert!(d.ends_with("L72.00 14.00"));
    }
}
