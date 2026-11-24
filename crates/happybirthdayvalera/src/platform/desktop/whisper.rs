//! whisper.cpp on one long-lived worker thread.
//!
//! The UI thread sends jobs over a std channel and awaits a oneshot per job, so the engine needs no async
//! runtime. The thread owns the model and runs jobs in order: an interim transcription and the final one
//! never run at the same time. Progress of a load goes to every caller that waits for a load.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;

use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_channel::oneshot;
use futures_util::{FutureExt as _, StreamExt as _, select};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use super::model::ModelStore;
use crate::engine::{EngineError, ModelSize, SpeechEngine};
use crate::lang::Lang;
use crate::resample::TARGET_RATE;

type Subscribers = Arc<Mutex<Vec<UnboundedSender<f32>>>>;

enum Job {
    Load { size: ModelSize, done: oneshot::Sender<Result<(), EngineError>> },
    Transcribe { pcm: Vec<f32>, lang: Lang, done: oneshot::Sender<Result<String, EngineError>> },
}

struct Worker {
    jobs: mpsc::Sender<Job>,
    // Kept so that the thread is owned; see the `Drop` impl for why it is not joined.
    _thread: JoinHandle<()>,
}

/// Whisper through whisper.cpp. Models download on first use into a [`ModelStore`].
pub struct WhisperEngine {
    store: ModelStore,
    worker: RefCell<Option<Worker>>,
    subscribers: Subscribers,
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperEngine {
    /// An engine over the default [`ModelStore`]. The worker thread starts on the first call.
    #[must_use]
    pub fn new() -> Self {
        Self::with_store(ModelStore::default())
    }

    /// An engine over `store`.
    #[must_use]
    pub fn with_store(store: ModelStore) -> Self {
        Self { store, worker: RefCell::new(None), subscribers: Subscribers::default() }
    }

    /// The store the engine downloads into.
    #[must_use]
    pub fn store(&self) -> &ModelStore {
        &self.store
    }

    fn submit(&self, job: Job) -> Result<(), EngineError> {
        let mut slot = self.worker.borrow_mut();
        if slot.is_none() {
            let (jobs, rx) = mpsc::channel();
            let store = self.store.clone();
            let subscribers = Arc::clone(&self.subscribers);
            let thread = std::thread::Builder::new()
                .name("hbv-whisper".into())
                .spawn(move || run(&rx, &store, &subscribers))
                .map_err(|e| EngineError::new(format!("cannot start the whisper thread: {e}")))?;
            *slot = Some(Worker { jobs, _thread: thread });
        }
        let worker = slot.as_ref().ok_or_else(|| EngineError::new("the whisper worker is missing"))?;
        if let Err(mpsc::SendError(job)) = worker.jobs.send(job) {
            // The thread is gone (it panicked). Forget it so that the next call starts a new one.
            *slot = None;
            drop(job);
            return Err(EngineError::new("the whisper thread stopped; try again"));
        }
        Ok(())
    }
}

impl Drop for WhisperEngine {
    fn drop(&mut self) {
        // Closing the job channel ends the thread after its current job. Joining here would block the UI
        // thread for as long as a transcription takes; a panic on that thread already reached its caller as
        // a closed reply channel, so nothing is lost by not joining.
        self.worker.get_mut().take();
    }
}

impl SpeechEngine for WhisperEngine {
    async fn load(&self, model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
        let (progress_tx, progress_rx) = unbounded();
        self.subscribers.lock().unwrap_or_else(PoisonError::into_inner).push(progress_tx);
        let (done_tx, done_rx) = oneshot::channel();
        self.submit(Job::Load { size: model, done: done_tx })?;
        let mut progress = progress_rx.fuse();
        let mut done = done_rx.fuse();
        loop {
            select! {
                fraction = progress.next() => if let Some(fraction) = fraction { on_progress(fraction) },
                result = done => {
                    return result.unwrap_or_else(|oneshot::Canceled| Err(EngineError::new("the whisper thread stopped")));
                }
            }
        }
    }

    async fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> Result<String, EngineError> {
        let (done_tx, done_rx) = oneshot::channel();
        self.submit(Job::Transcribe { pcm, lang, done: done_tx })?;
        done_rx.await.unwrap_or_else(|oneshot::Canceled| Err(EngineError::new("the whisper thread stopped")))
    }

    fn dispose(&self) {
        self.worker.borrow_mut().take();
    }
}

struct Loaded {
    size: ModelSize,
    // The state holds an `Arc` of the context, so the context needs no field of its own.
    state: WhisperState,
}

fn run(jobs: &mpsc::Receiver<Job>, store: &ModelStore, subscribers: &Subscribers) {
    whisper_rs::install_logging_hooks();
    let mut loaded: Option<Loaded> = None;
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Load { size, done } => {
                let result = if loaded.as_ref().is_some_and(|l| l.size == size) {
                    broadcast(subscribers, 1.0);
                    Ok(())
                } else {
                    // Free the old model first: two models at once can need a gigabyte.
                    loaded = None;
                    load(store, size, subscribers).map(|state| loaded = Some(Loaded { size, state }))
                };
                // The caller may have stopped waiting (the listener stopped); the model stays loaded anyway.
                let _ = done.send(result);
                subscribers.lock().unwrap_or_else(PoisonError::into_inner).retain(|tx| !tx.is_closed());
            }
            Job::Transcribe { pcm, lang, done } => {
                let result = match loaded.as_mut() {
                    Some(l) => transcribe(&mut l.state, &pcm, lang),
                    None => Err(EngineError::new("transcribe called before load")),
                };
                // A closed channel means the listener moved on; the text is no longer wanted.
                let _ = done.send(result);
            }
        }
    }
}

fn broadcast(subscribers: &Subscribers, fraction: f32) {
    subscribers.lock().unwrap_or_else(PoisonError::into_inner).retain(|tx| tx.unbounded_send(fraction).is_ok());
}

fn load(store: &ModelStore, size: ModelSize, subscribers: &Subscribers) -> Result<WhisperState, EngineError> {
    let path = store.ensure(size, &mut |fraction| broadcast(subscribers, fraction))?;
    let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())
        .map_err(|e| EngineError::new(format!("cannot load {}: {e}", path.display())))?;
    ctx.create_state().map_err(|e| EngineError::new(format!("cannot create a whisper state: {e}")))
}

/// Whisper warns about and handles badly any input under one second, so short recordings get padded.
const MIN_SAMPLES: usize = TARGET_RATE as usize + TARGET_RATE as usize / 10;

fn transcribe(state: &mut WhisperState, pcm: &[f32], lang: Lang) -> Result<String, EngineError> {
    let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
    params.set_language(Some(lang.code()));
    params.set_translate(false);
    params.set_no_context(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    params.set_n_threads(i32::try_from(threads).unwrap_or(4));

    let mut padded;
    let audio = if pcm.len() < MIN_SAMPLES {
        padded = pcm.to_vec();
        padded.resize(MIN_SAMPLES, 0.0);
        &padded[..]
    } else {
        pcm
    };
    state.full(params, audio).map_err(|e| EngineError::new(format!("whisper failed: {e}")))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        let part = segment.to_str_lossy().map_err(|e| EngineError::new(format!("cannot read a segment: {e}")))?;
        text.push_str(&part);
    }
    Ok(text.trim().to_owned())
}
