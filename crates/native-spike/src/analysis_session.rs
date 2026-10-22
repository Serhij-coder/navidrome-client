use super::{Capture, Error};
use feishin_spike_model::Analysis;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

/// Session-scoped consent. Re-enabling a consumer reuses only this explicit source.
#[derive(Clone)]
pub enum AnalysisSource {
    TestTone,
    Monitor(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisStatus {
    Suspended,
    Starting,
    Running,
    Failed,
    Stopped,
}

struct State {
    active: bool,
    stopped: bool,
    generation: u64,
    status: AnalysisStatus,
    latest: Option<Analysis>,
}

/// Bounded capture supervisor. All process spawn/stop/pipe work stays off the UI.
/// Failures are latched until an explicit disable/enable or retry request.
pub struct AnalysisSession {
    shared: Arc<(Mutex<State>, Condvar)>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl AnalysisSession {
    pub fn new(
        ffmpeg: PathBuf,
        source: AnalysisSource,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, Error> {
        if let AnalysisSource::Monitor(name) = &source {
            super::capture::validate_monitor(name)?;
        }
        let shared = Arc::new((
            Mutex::new(State {
                active: false,
                stopped: false,
                generation: 0,
                status: AnalysisStatus::Suspended,
                latest: None,
            }),
            Condvar::new(),
        ));
        let session = shared.clone();
        let worker = std::thread::Builder::new()
            .name("p01-analysis".into())
            .spawn(move || {
                let (lock, changed) = &*session;
                let mut capture = None;
                let mut generation = 0;
                let mut failed = false;
                loop {
                    let state = lock.lock().unwrap_or_else(|error| error.into_inner());
                    if state.stopped {
                        break;
                    }
                    if !state.active || state.generation != generation {
                        generation = state.generation;
                        failed = false;
                        drop(state);
                        // Drop waits for owned child/pipe cleanup on this worker only.
                        capture = None;
                        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                        state.latest = None;
                        if !state.active && !state.stopped {
                            state.status = AnalysisStatus::Suspended;
                            drop(state);
                            wake();
                            let state = lock.lock().unwrap_or_else(|error| error.into_inner());
                            drop(
                                changed
                                    .wait_while(state, |state| !state.active && !state.stopped)
                                    .unwrap_or_else(|error| error.into_inner()),
                            );
                        }
                        continue;
                    }
                    drop(state);
                    if capture.is_none() && !failed {
                        {
                            let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                            state.status = AnalysisStatus::Starting;
                        }
                        wake();
                        let result = match &source {
                            AnalysisSource::TestTone => Capture::start_test_tone(&ffmpeg),
                            AnalysisSource::Monitor(name) => Capture::start_monitor(&ffmpeg, name),
                        };
                        match result {
                            Ok(started) => capture = Some(started),
                            Err(_) => failed = true,
                        }
                    }
                    let mut latest = None;
                    if let Some(current) = &mut capture {
                        if matches!(current.is_running(), Ok(true)) {
                            latest = current.latest();
                        } else {
                            capture = None;
                            failed = true;
                        }
                    }
                    let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                    // A source that completes after suspension/retry cannot publish an old frame.
                    let mut notify = false;
                    if state.active && !state.stopped && state.generation == generation {
                        let status = if failed {
                            AnalysisStatus::Failed
                        } else {
                            AnalysisStatus::Running
                        };
                        notify = latest.is_some() || state.status != status;
                        state.status = status;
                        if failed {
                            state.latest = None;
                        } else if latest.is_some() {
                            state.latest = latest;
                        }
                    }
                    drop(state);
                    if notify {
                        wake();
                    }
                    let state = lock.lock().unwrap_or_else(|error| error.into_inner());
                    if !state.active || state.stopped || state.generation != generation {
                        continue;
                    }
                    if failed {
                        drop(
                            changed
                                .wait_while(state, |state| {
                                    state.active && !state.stopped && state.generation == generation
                                })
                                .unwrap_or_else(|error| error.into_inner()),
                        );
                    } else {
                        drop(
                            changed
                                .wait_timeout(state, Duration::from_millis(33))
                                .unwrap_or_else(|error| error.into_inner()),
                        );
                    }
                }
                drop(capture);
                let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                state.latest = None;
                state.status = AnalysisStatus::Stopped;
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Nonblocking control; invalidates queued frames before returning.
    pub fn set_active(&self, active: bool) {
        let mut state = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.active != active && !state.stopped {
            state.active = active;
            state.generation = state.generation.wrapping_add(1);
            state.latest = None;
            self.shared.1.notify_one();
        }
    }

    pub fn retry(&self) {
        let mut state = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.active && !state.stopped {
            state.generation = state.generation.wrapping_add(1);
            state.latest = None;
            self.shared.1.notify_one();
        }
    }

    pub fn latest(&self) -> Option<Analysis> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .latest
            .take()
    }

    pub fn status(&self) -> AnalysisStatus {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .status
    }

    /// Blocking final cleanup. Call outside an update/async executor.
    pub fn shutdown(&mut self) -> Result<(), Error> {
        {
            let mut state = self
                .shared
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.stopped = true;
            state.active = false;
            state.latest = None;
            self.shared.1.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| Error::Protocol)?;
        }
        Ok(())
    }
}

impl Drop for AnalysisSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
