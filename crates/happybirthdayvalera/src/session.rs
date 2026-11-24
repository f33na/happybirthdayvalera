//! The pure session logic behind the controller: segmenting, drafts, the engine queue and the auto-stop.
//!
//! Captured audio is the clock. Every duration counts samples, so the logic needs no timer, and the tests drive
//! it with fake audio blocks and fake engine results. One [`Session`] lives from `listening` until the queued
//! segments are committed.

use std::collections::VecDeque;

use crate::clean::clean_transcript;
use crate::engine::EngineError;
use crate::listener::Mode;
use crate::resample::TARGET_RATE;
use crate::vad::rms;

/// Recordings shorter than this skip the engine: Whisper invents text from such short audio.
pub(crate) const MIN_TRANSCRIBE_SAMPLES: usize = TARGET_RATE as usize * 3 / 10;

/// The numbers of one session, in milliseconds unless the name says otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SessionConfig {
    pub(crate) mode: Mode,
    pub(crate) threshold_rms: f32,
    pub(crate) min_speech_ms: u32,
    /// Utterance mode: after speech, this much silence ends the utterance.
    pub(crate) silence_ms: u32,
    /// Continuous mode: after speech, this much silence ends a segment.
    pub(crate) segment_silence_ms: u32,
    pub(crate) auto_stop_ms: u32,
    pub(crate) pad_before_ms: u32,
    pub(crate) pad_after_ms: u32,
    pub(crate) max_segment_ms: u32,
    /// Utterance mode: the recording stops at this length.
    pub(crate) max_duration_ms: u32,
    /// `0` turns drafts off.
    pub(crate) interim_every_ms: u32,
}

fn samples(ms: u32) -> usize {
    TARGET_RATE as usize * ms as usize / 1000
}

/// What an engine call is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JobKind {
    /// The audio of a closed segment; its result is committed.
    Final,
    /// The audio of the open segment so far; its result is the draft.
    Draft,
}

/// One engine call. `id` ties the result to the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Job {
    pub(crate) id: u32,
    pub(crate) segment: u32,
    pub(crate) kind: JobKind,
}

/// Why the session leaves `listening` on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Leave {
    /// Utterance mode: the silence after speech. The machine gets `SILENCE`.
    Silence,
    /// The utterance reached its maximum length, or the continuous auto-stop. The machine gets `STOP`.
    Stop,
}

/// What the controller must do, in order.
#[derive(Debug, PartialEq)]
pub(crate) enum Effect {
    /// Start one engine call; its result goes back through [`Session::engine_done`].
    Transcribe { job: Job, pcm: Vec<f32> },
    /// The draft of `segment`; an empty text clears it.
    Interim { segment: u32, text: String },
    /// The final, non-empty, trimmed text of `segment`. Segments commit in order.
    Commit { segment: u32, text: String },
    /// Leave `listening` and release the microphone.
    Leave(Leave),
    /// Every segment is committed after the stop. `text` is the session text; `failed` is true when an engine
    /// call for a segment failed.
    Done { text: String, failed: bool },
}

#[derive(Clone, Copy, Debug)]
struct Open {
    id: u32,
    /// The first sample of the segment audio, padding included (absolute index).
    start: usize,
    /// The end of the last block at or above the threshold (absolute index).
    speech_end: usize,
}

struct Pending {
    segment: u32,
    /// `None` for audio too short for the engine: the segment resolves as empty, in its turn.
    pcm: Option<Vec<f32>>,
}

pub(crate) struct Session {
    cfg: SessionConfig,
    /// The kept audio; `buf[0]` is the absolute sample `origin`.
    buf: Vec<f32>,
    origin: usize,
    now: usize,
    above: usize,
    below: usize,
    open: Option<Open>,
    last_speech_end: usize,
    next_segment: u32,
    /// The next segment to commit; segments commit in order, so lower ids are done.
    next_commit: u32,
    /// The end of the last closed segment: a new segment never starts before it, so no audio goes twice.
    last_end: usize,
    queue: VecDeque<Pending>,
    busy: Option<Job>,
    next_job: u32,
    since_draft: usize,
    interim_segment: Option<u32>,
    text: String,
    failed: bool,
    stopping: bool,
    done: bool,
}

/// The buffer drops audio that no segment can need any more once this much of it piles up.
const TRIM_SAMPLES: usize = TARGET_RATE as usize * 5;

impl Session {
    pub(crate) fn new(cfg: SessionConfig) -> Self {
        Self {
            cfg,
            buf: Vec::new(),
            origin: 0,
            now: 0,
            above: 0,
            below: 0,
            open: None,
            last_speech_end: 0,
            next_segment: 0,
            next_commit: 0,
            last_end: 0,
            queue: VecDeque::new(),
            busy: None,
            next_job: 0,
            since_draft: 0,
            interim_segment: None,
            text: String::new(),
            failed: false,
            stopping: false,
            done: false,
        }
    }

    /// The committed text so far, segments joined with one space.
    #[cfg(test)]
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Feeds one block of 16 kHz mono audio.
    pub(crate) fn audio(&mut self, block: &[f32]) -> Vec<Effect> {
        let mut fx = Vec::new();
        if self.stopping {
            return fx;
        }
        let n = block.len();
        let loud = rms(block) >= self.cfg.threshold_rms;
        self.buf.extend_from_slice(block);
        self.now += n;
        if loud {
            self.above += n;
            self.below = 0;
        } else {
            self.below += n;
            self.above = 0;
        }

        match &mut self.open {
            None if self.above >= samples(self.cfg.min_speech_ms) => {
                let speech_start = self.now - self.above;
                let start = match self.cfg.mode {
                    // The utterance is the whole recording, as before segments existed.
                    Mode::Utterance => 0,
                    Mode::Continuous => speech_start
                        .saturating_sub(samples(self.cfg.pad_before_ms))
                        .max(self.origin)
                        .max(self.last_end),
                };
                self.open_segment(start);
            }
            Some(open) if loud => {
                open.speech_end = self.now;
                self.last_speech_end = self.now;
            }
            None | Some(_) => {}
        }

        if let Some(open) = self.open {
            match self.cfg.mode {
                Mode::Utterance => {
                    if self.below >= samples(self.cfg.silence_ms) {
                        self.close(self.now);
                        self.leave(Leave::Silence, &mut fx);
                    }
                }
                Mode::Continuous => {
                    if self.below >= samples(self.cfg.segment_silence_ms) {
                        let end = (open.speech_end + samples(self.cfg.pad_after_ms)).min(self.now);
                        self.close(end);
                    } else if self.now - open.start >= samples(self.cfg.max_segment_ms) {
                        // Whisper sees at most 30 s. The speech goes on, so the next segment starts right here,
                        // without padding, which would repeat audio of this one.
                        self.close(self.now);
                        self.open_segment(self.now);
                    }
                }
            }
        }

        if !self.stopping {
            match self.cfg.mode {
                Mode::Utterance => {
                    if self.now >= samples(self.cfg.max_duration_ms) {
                        if self.open.is_some() {
                            self.close(self.now);
                        }
                        self.leave(Leave::Stop, &mut fx);
                    }
                }
                Mode::Continuous => {
                    if self.open.is_none() && self.now - self.last_speech_end >= samples(self.cfg.auto_stop_ms) {
                        self.leave(Leave::Stop, &mut fx);
                    }
                }
            }
        }

        if !self.stopping && self.open.is_some() && self.cfg.interim_every_ms > 0 {
            self.since_draft += n;
            if self.since_draft >= samples(self.cfg.interim_every_ms) {
                self.since_draft = 0;
                self.draft(&mut fx);
            }
        }

        self.trim();
        self.pump(&mut fx);
        fx
    }

    /// `STOP` while listening: closes the open segment and drains the queue. The controller leaves
    /// `listening` itself.
    pub(crate) fn stop(&mut self) -> Vec<Effect> {
        let mut fx = Vec::new();
        if self.stopping {
            return fx;
        }
        if let Some(open) = self.open {
            let end = match self.cfg.mode {
                Mode::Utterance => self.now,
                Mode::Continuous => (open.speech_end + samples(self.cfg.pad_after_ms)).min(self.now),
            };
            self.close(end);
        }
        self.stopping = true;
        self.pump(&mut fx);
        fx
    }

    /// The result of the engine call `job_id`. A result of an unknown call is dropped.
    pub(crate) fn engine_done(&mut self, job_id: u32, result: Result<String, EngineError>) -> Vec<Effect> {
        let mut fx = Vec::new();
        let Some(job) = self.busy.filter(|job| job.id == job_id) else { return fx };
        self.busy = None;
        match job.kind {
            JobKind::Draft => {
                // A draft that returns after its segment closed still shows until that segment commits: the
                // final call waited for this draft, so with a slow model it is often the only draft.
                let current = job.segment >= self.next_commit;
                match result {
                    Ok(text) if current => {
                        // An empty draft keeps the previous one: silence must not blank what was said.
                        let text = clean_transcript(&text);
                        if !text.is_empty() {
                            self.interim_segment = Some(job.segment);
                            fx.push(Effect::Interim { segment: job.segment, text });
                        }
                    }
                    Ok(_) => {}
                    Err(e) => tracing::debug!("a draft transcription failed: {e}"),
                }
            }
            JobKind::Final => self.finish_segment(job.segment, result, &mut fx),
        }
        self.pump(&mut fx);
        fx
    }

    fn open_segment(&mut self, start: usize) {
        self.open = Some(Open { id: self.next_segment, start, speech_end: self.now });
        self.next_segment = self.next_segment.wrapping_add(1);
        self.last_speech_end = self.now;
        self.since_draft = 0;
    }

    /// Queues the open segment with its audio up to `end`.
    fn close(&mut self, end: usize) {
        let Some(open) = self.open.take() else { return };
        self.last_end = end;
        let from = open.start.saturating_sub(self.origin);
        let to = end.saturating_sub(self.origin).min(self.buf.len());
        let pcm = (to.saturating_sub(from) >= MIN_TRANSCRIBE_SAMPLES).then(|| self.buf[from..to].to_vec());
        self.queue.push_back(Pending { segment: open.id, pcm });
    }

    fn leave(&mut self, why: Leave, fx: &mut Vec<Effect>) {
        self.stopping = true;
        fx.push(Effect::Leave(why));
    }

    /// A draft over the open segment, unless an engine call runs or a final call waits.
    fn draft(&mut self, fx: &mut Vec<Effect>) {
        let Some(open) = self.open else { return };
        if self.busy.is_some() || !self.queue.is_empty() {
            return;
        }
        let from = open.start.saturating_sub(self.origin);
        if self.buf.len().saturating_sub(from) < MIN_TRANSCRIBE_SAMPLES {
            return;
        }
        let pcm = self.buf[from..].to_vec();
        self.call(open.id, JobKind::Draft, pcm, fx);
    }

    fn call(&mut self, segment: u32, kind: JobKind, pcm: Vec<f32>, fx: &mut Vec<Effect>) {
        let job = Job { id: self.next_job, segment, kind };
        self.next_job = self.next_job.wrapping_add(1);
        self.busy = Some(job);
        fx.push(Effect::Transcribe { job, pcm });
    }

    /// Starts the next final call when the engine is free; reports `Done` once a stopped session drained.
    fn pump(&mut self, fx: &mut Vec<Effect>) {
        while self.busy.is_none() {
            match self.queue.pop_front() {
                Some(Pending { segment, pcm: Some(pcm) }) => self.call(segment, JobKind::Final, pcm, fx),
                Some(Pending { segment, pcm: None }) => self.finish_segment(segment, Ok(String::new()), fx),
                None => {
                    if self.stopping && !self.done {
                        self.done = true;
                        fx.push(Effect::Done { text: self.text.clone(), failed: self.failed });
                    }
                    return;
                }
            }
        }
    }

    fn finish_segment(&mut self, segment: u32, result: Result<String, EngineError>, fx: &mut Vec<Effect>) {
        self.next_commit = segment.wrapping_add(1);
        match result {
            Ok(text) => {
                let text = clean_transcript(&text);
                if !text.is_empty() {
                    if !self.text.is_empty() {
                        self.text.push(' ');
                    }
                    self.text.push_str(&text);
                    fx.push(Effect::Commit { segment, text });
                }
            }
            Err(e) => {
                tracing::warn!("transcribing segment {segment} failed: {e}");
                self.failed = true;
            }
        }
        if self.interim_segment == Some(segment) {
            self.interim_segment = None;
            fx.push(Effect::Interim { segment, text: String::new() });
        }
    }

    /// Drops audio that neither the open segment nor the padding of a future one can need.
    fn trim(&mut self) {
        if self.cfg.mode == Mode::Utterance {
            return;
        }
        let keep_from = match self.open {
            Some(open) => open.start,
            None => self.now.saturating_sub(self.above + samples(self.cfg.pad_before_ms)),
        };
        let drop = keep_from.saturating_sub(self.origin);
        if drop >= TRIM_SAMPLES {
            self.buf.drain(..drop);
            self.origin = keep_from;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 320; // 20 ms
    const LOUD: f32 = 0.05;
    const QUIET: f32 = 0.001;

    fn cfg() -> SessionConfig {
        SessionConfig {
            mode: Mode::Continuous,
            threshold_rms: 0.012,
            min_speech_ms: 250,
            silence_ms: 1500,
            segment_silence_ms: 700,
            auto_stop_ms: 8000,
            pad_before_ms: 200,
            pad_after_ms: 300,
            max_segment_ms: 28_000,
            max_duration_ms: 30_000,
            interim_every_ms: 1000,
        }
    }

    fn ms(n: usize) -> usize {
        n * 16
    }

    /// Feeds `ms` of audio at `level` in 20 ms blocks and collects the effects.
    fn feed(s: &mut Session, level: f32, ms: usize) -> Vec<Effect> {
        let mut fx = Vec::new();
        for _ in 0..ms / 20 {
            fx.extend(s.audio(&[level; BLOCK]));
        }
        fx
    }

    fn calls(fx: &[Effect]) -> Vec<(Job, usize)> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Transcribe { job, pcm } => Some((*job, pcm.len())),
                _ => None,
            })
            .collect()
    }

    fn finals(fx: &[Effect]) -> Vec<(Job, Vec<f32>)> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Transcribe { job, pcm } if job.kind == JobKind::Final => Some((*job, pcm.clone())),
                _ => None,
            })
            .collect()
    }

    fn no_drafts() -> SessionConfig {
        SessionConfig { interim_every_ms: 0, ..cfg() }
    }

    #[test]
    fn a_segment_goes_to_the_engine_with_padding() {
        let mut s = Session::new(no_drafts());
        let mut fx = feed(&mut s, QUIET, 1000);
        fx.extend(feed(&mut s, LOUD, 1000));
        assert!(calls(&fx).is_empty(), "the segment is still open");
        fx = feed(&mut s, QUIET, 700);
        let f = finals(&fx);
        assert_eq!(f.len(), 1, "700 ms of silence closes the segment");
        let pcm = &f[0].1;
        // Speech ran 1000..2000 ms: 200 ms before it and 300 ms after it go along.
        assert_eq!(pcm.len(), ms(1500));
        assert!(pcm[..ms(200)].iter().all(|&x| x == QUIET));
        assert!(pcm[ms(200)..ms(1200)].iter().all(|&x| x == LOUD));
        assert!(pcm[ms(1200)..].iter().all(|&x| x == QUIET));
    }

    #[test]
    fn padding_is_clamped_to_the_captured_audio() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 500);
        let f = finals(&feed(&mut s, QUIET, 700));
        // No audio before the start of listening: the segment starts at 0.
        assert_eq!(f[0].1.len(), ms(800));
        assert!(f[0].1[..ms(500)].iter().all(|&x| x == LOUD));
    }

    #[test]
    fn a_short_blip_opens_no_segment() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 200);
        assert!(calls(&feed(&mut s, QUIET, 2000)).is_empty());
        assert_eq!(s.stop(), vec![Effect::Done { text: String::new(), failed: false }]);
    }

    #[test]
    fn segments_commit_in_order_one_call_at_a_time() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 500);
        let first = finals(&feed(&mut s, QUIET, 700));
        assert_eq!(first.len(), 1);
        feed(&mut s, LOUD, 500);
        let fx = feed(&mut s, QUIET, 700);
        assert!(calls(&fx).is_empty(), "the second final waits for the first call");

        let fx = s.engine_done(first[0].0.id, Ok(" One. ".into()));
        assert_eq!(fx[0], Effect::Commit { segment: 0, text: "One.".into() });
        let second = finals(&fx);
        assert_eq!(second.len(), 1, "the next final starts when the first is done");
        assert_eq!(second[0].0.segment, 1);

        let fx = s.engine_done(second[0].0.id, Ok("Two.".into()));
        assert_eq!(fx, vec![Effect::Commit { segment: 1, text: "Two.".into() }]);
        assert_eq!(s.text(), "One. Two.");
    }

    #[test]
    fn an_empty_segment_commits_nothing() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 500);
        let f = finals(&feed(&mut s, QUIET, 700));
        assert!(s.engine_done(f[0].0.id, Ok("  ".into())).is_empty());
        assert_eq!(s.text(), "");
    }

    #[test]
    fn a_result_of_an_unknown_call_is_dropped() {
        let mut s = Session::new(no_drafts());
        assert!(s.engine_done(42, Ok("ghost".into())).is_empty());
    }

    #[test]
    fn drafts_cover_only_the_open_segment() {
        let mut s = Session::new(cfg());
        feed(&mut s, QUIET, 3000);
        let fx = feed(&mut s, LOUD, 1300);
        let drafts = calls(&fx);
        assert_eq!(drafts.len(), 1);
        let (job, len) = drafts[0];
        assert_eq!(job.kind, JobKind::Draft);
        // The segment opened at 3250 ms with speech from 3000 ms, so its audio starts at 2800 ms; the draft
        // fired 1000 ms of audio after the opening block ended, at 4240 ms.
        assert_eq!(len, ms(4240 - 2800));
        let fx = s.engine_done(job.id, Ok("hello wor".into()));
        assert_eq!(fx, vec![Effect::Interim { segment: 0, text: "hello wor".into() }]);
    }

    #[test]
    fn a_draft_of_a_later_segment_does_not_hold_earlier_audio() {
        let mut s = Session::new(cfg());
        feed(&mut s, LOUD, 500);
        let f = finals(&feed(&mut s, QUIET, 700));
        s.engine_done(f[0].0.id, Ok("one".into()));
        let drafts = calls(&feed(&mut s, LOUD, 1300));
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].0.segment, 1);
        // Only the second segment: 200 ms of padding plus its speech up to the tick.
        assert!(drafts[0].1 < ms(1600), "draft of {} samples", drafts[0].1);
    }

    #[test]
    fn a_draft_tick_is_skipped_while_a_final_runs_or_waits() {
        let mut s = Session::new(cfg());
        feed(&mut s, LOUD, 500);
        let f = finals(&feed(&mut s, QUIET, 700));
        assert_eq!(f.len(), 1);
        // Seg 1 opens and speaks long enough for two ticks; the final of seg 0 still runs.
        assert!(calls(&feed(&mut s, LOUD, 2500)).is_empty());
        // Seg 1 closes and its final waits behind seg 0; seg 2 speaks, and its ticks still skip.
        feed(&mut s, QUIET, 700);
        assert!(calls(&feed(&mut s, LOUD, 2500)).is_empty());
        // Seg 0 is done: the queued final of seg 1 goes first, not a draft.
        let fx = s.engine_done(f[0].0.id, Ok("zero".into()));
        let next = calls(&fx);
        assert_eq!(next.len(), 1);
        assert_eq!((next[0].0.kind, next[0].0.segment), (JobKind::Final, 1));
    }

    #[test]
    fn a_draft_tick_is_skipped_while_a_draft_runs() {
        let mut s = Session::new(cfg());
        let first = calls(&feed(&mut s, LOUD, 1300));
        assert_eq!(first.len(), 1);
        assert!(calls(&feed(&mut s, LOUD, 1000)).is_empty(), "the first draft has not returned");
        s.engine_done(first[0].0.id, Ok("hi".into()));
        assert_eq!(calls(&feed(&mut s, LOUD, 1000)).len(), 1);
    }

    #[test]
    fn a_draft_that_returns_after_its_segment_closed_shows_until_the_commit() {
        let mut s = Session::new(cfg());
        let d = calls(&feed(&mut s, LOUD, 1300));
        let fx = feed(&mut s, QUIET, 700);
        assert!(calls(&fx).is_empty(), "the final waits for the running draft");
        let fx = s.engine_done(d[0].0.id, Ok("late".into()));
        assert_eq!(fx[0], Effect::Interim { segment: 0, text: "late".into() });
        let f = finals(&fx);
        assert_eq!(f.len(), 1, "the waiting final starts");
        let fx = s.engine_done(f[0].0.id, Ok("Late.".into()));
        assert_eq!(fx[1], Effect::Interim { segment: 0, text: String::new() }, "the commit clears it");
    }

    #[test]
    fn a_segment_never_starts_before_the_previous_one_ends() {
        let mut s = Session::new(SessionConfig { segment_silence_ms: 300, pad_before_ms: 400, ..no_drafts() });
        feed(&mut s, LOUD, 500);
        let first = finals(&feed(&mut s, QUIET, 300));
        // The first segment ends at 800 ms (500 + 300 of padding, clamped to the 800 captured).
        assert_eq!(first[0].1.len(), ms(800));
        s.engine_done(first[0].0.id, Ok("a".into()));
        feed(&mut s, LOUD, 500);
        let second = finals(&feed(&mut s, QUIET, 300));
        // Speech from 800 ms; 400 ms of padding would reach back to 400 ms, but the segment starts at 800 ms.
        assert_eq!(second[0].1.len(), ms(500 + 300));
        assert!(second[0].1[..ms(500)].iter().all(|&x| x == LOUD));
    }

    #[test]
    fn the_draft_clears_when_its_segment_commits_or_turns_out_empty() {
        let mut s = Session::new(cfg());
        let d = calls(&feed(&mut s, LOUD, 1300));
        s.engine_done(d[0].0.id, Ok("hel".into()));
        let f = finals(&feed(&mut s, QUIET, 700));
        let fx = s.engine_done(f[0].0.id, Ok("Hello.".into()));
        assert_eq!(
            fx,
            vec![
                Effect::Commit { segment: 0, text: "Hello.".into() },
                Effect::Interim { segment: 0, text: String::new() },
            ]
        );

        let d = calls(&feed(&mut s, LOUD, 1300));
        s.engine_done(d[0].0.id, Ok("uh".into()));
        let f = finals(&feed(&mut s, QUIET, 700));
        assert_eq!(s.engine_done(f[0].0.id, Ok(String::new())), vec![Effect::Interim { segment: 1, text: String::new() }]);
    }

    #[test]
    fn a_non_speech_draft_keeps_the_previous_draft() {
        let mut s = Session::new(cfg());
        let d = calls(&feed(&mut s, LOUD, 1300));
        assert_eq!(s.engine_done(d[0].0.id, Ok("hello".into())), vec![Effect::Interim { segment: 0, text: "hello".into() }]);
        let d = calls(&feed(&mut s, LOUD, 1000));
        assert!(s.engine_done(d[0].0.id, Ok(" [BLANK_AUDIO] ".into())).is_empty(), "no effect: the draft stays");
        let d = calls(&feed(&mut s, LOUD, 1000));
        assert_eq!(
            s.engine_done(d[0].0.id, Ok("[Music] hello there".into())),
            vec![Effect::Interim { segment: 0, text: "hello there".into() }]
        );
    }

    #[test]
    fn a_non_speech_final_commits_nothing() {
        let mut s = Session::new(cfg());
        let d = calls(&feed(&mut s, LOUD, 1300));
        s.engine_done(d[0].0.id, Ok("hel".into()));
        let f = finals(&feed(&mut s, QUIET, 700));
        assert_eq!(
            s.engine_done(f[0].0.id, Ok("Субтитры сделал DimaTorzok".into())),
            vec![Effect::Interim { segment: 0, text: String::new() }],
            "no commit; the draft clears"
        );
        assert_eq!(s.text(), "");
    }

    #[test]
    fn no_speech_for_auto_stop_ms_stops() {
        let mut s = Session::new(no_drafts());
        assert!(feed(&mut s, QUIET, 7980).is_empty());
        let fx = feed(&mut s, QUIET, 20);
        assert_eq!(fx, vec![Effect::Leave(Leave::Stop), Effect::Done { text: String::new(), failed: false }]);
        assert!(feed(&mut s, LOUD, 1000).is_empty(), "audio after the stop is ignored");
    }

    #[test]
    fn the_auto_stop_counts_from_the_end_of_the_last_speech() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, QUIET, 5000);
        feed(&mut s, LOUD, 500);
        let fx = feed(&mut s, QUIET, 7980);
        assert!(!fx.contains(&Effect::Leave(Leave::Stop)));
        assert!(feed(&mut s, QUIET, 20).contains(&Effect::Leave(Leave::Stop)));
    }

    #[test]
    fn stop_closes_the_open_segment_and_drains_the_queue() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 500);
        let first = finals(&feed(&mut s, QUIET, 700));
        feed(&mut s, LOUD, 600);
        let fx = s.stop();
        assert!(fx.is_empty(), "the open segment queues behind the running call; no Done yet");
        let fx = s.engine_done(first[0].0.id, Ok("one".into()));
        let second = finals(&fx);
        assert_eq!(second.len(), 1);
        // Stopped mid-speech: the segment ends at the stop, with no padding after it.
        assert_eq!(second[0].1.len(), ms(200 + 600));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Done { .. })));
        let fx = s.engine_done(second[0].0.id, Ok("two".into()));
        assert_eq!(fx.last(), Some(&Effect::Done { text: "one two".into(), failed: false }));
        assert!(s.stop().is_empty(), "a second stop does nothing");
    }

    #[test]
    fn stop_waits_for_a_running_draft() {
        let mut s = Session::new(cfg());
        let d = calls(&feed(&mut s, LOUD, 1300));
        assert!(s.stop().is_empty());
        let fx = s.engine_done(d[0].0.id, Ok("dra".into()));
        assert_eq!(fx[0], Effect::Interim { segment: 0, text: "dra".into() });
        let f = finals(&fx);
        let fx = s.engine_done(f[0].0.id, Ok("Draft.".into()));
        assert_eq!(fx.last(), Some(&Effect::Done { text: "Draft.".into(), failed: false }));
    }

    #[test]
    fn audio_too_short_for_the_engine_is_an_empty_segment() {
        let mut s = Session::new(SessionConfig { pad_before_ms: 0, pad_after_ms: 0, ..no_drafts() });
        feed(&mut s, LOUD, 260);
        // 260 ms of audio is under the 300 ms floor: no engine call.
        assert_eq!(s.stop(), vec![Effect::Done { text: String::new(), failed: false }]);
    }

    #[test]
    fn a_long_segment_is_cut_at_max_segment_ms() {
        let mut s = Session::new(SessionConfig { max_segment_ms: 3000, ..no_drafts() });
        let f = finals(&feed(&mut s, LOUD, 3000));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].1.len(), ms(3000));
        s.engine_done(f[0].0.id, Ok("a".into()));
        let f = finals(&feed(&mut s, QUIET, 700));
        assert_eq!(f[0].0.segment, 1, "the speech goes on in the next segment");
    }

    #[test]
    fn a_failed_segment_is_skipped_and_reported_at_the_end() {
        let mut s = Session::new(no_drafts());
        feed(&mut s, LOUD, 500);
        let f = finals(&feed(&mut s, QUIET, 700));
        assert!(s.engine_done(f[0].0.id, Err(EngineError::new("boom"))).is_empty());
        assert_eq!(s.stop(), vec![Effect::Done { text: String::new(), failed: true }]);
    }

    #[test]
    fn utterance_mode_ends_on_silence_with_the_whole_recording() {
        let mut s = Session::new(SessionConfig { mode: Mode::Utterance, ..no_drafts() });
        feed(&mut s, QUIET, 1000);
        feed(&mut s, LOUD, 500);
        assert!(!feed(&mut s, QUIET, 1480).contains(&Effect::Leave(Leave::Silence)));
        let fx = feed(&mut s, QUIET, 20);
        assert_eq!(fx[0], Effect::Leave(Leave::Silence));
        let f = finals(&fx);
        assert_eq!(f[0].1.len(), ms(3000), "from the start of listening to the end");
        let fx = s.engine_done(f[0].0.id, Ok("Hi.".into()));
        assert_eq!(
            fx,
            vec![Effect::Commit { segment: 0, text: "Hi.".into() }, Effect::Done { text: "Hi.".into(), failed: false }]
        );
    }

    #[test]
    fn utterance_mode_has_no_auto_stop_but_a_maximum_length() {
        let mut s = Session::new(SessionConfig { mode: Mode::Utterance, max_duration_ms: 10_000, ..no_drafts() });
        assert!(feed(&mut s, QUIET, 9980).is_empty());
        assert_eq!(feed(&mut s, QUIET, 20), vec![Effect::Leave(Leave::Stop), Effect::Done { text: String::new(), failed: false }]);
    }

    #[test]
    fn utterance_drafts_cover_the_whole_utterance() {
        let mut s = Session::new(SessionConfig { mode: Mode::Utterance, ..cfg() });
        feed(&mut s, QUIET, 2000);
        let d = calls(&feed(&mut s, LOUD, 1300));
        assert_eq!(d[0].1, ms(2000 + 1240));
    }
}
