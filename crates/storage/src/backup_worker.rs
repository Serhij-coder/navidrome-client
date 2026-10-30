//! One-owner, one-in-flight blocking backup worker. Drop never joins a filesystem call.
use crate::{
    AtomicStore, NativeStore, backup_executor as executor,
    playlist_backups::{Backup, Repository},
};
use feishin_domain::*;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    task::{Context, Poll, Waker},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
type Result = std::result::Result<Revision, BackupFailure>;
#[derive(Default)]
struct Completion {
    state: Mutex<(Option<Result>, Option<Waker>)>,
    cancelled: AtomicBool,
}
impl Completion {
    fn complete(&self, result: Result) {
        let waker = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.0 = Some(result);
            state.1.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
struct Wait<'a> {
    completion: Arc<Completion>,
    clock: &'a dyn Clock,
    context: OperationContext<'a>,
}
impl Future for Wait<'_> {
    type Output = Result;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result> {
        if let Err(error) = self.context.check(self.clock.now()) {
            return Poll::Ready(Err(executor::failure(BackupError::Context(error), true)));
        }
        let mut state = self
            .completion
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(result) = state.0.take() {
            Poll::Ready(result)
        } else {
            state.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for Wait<'_> {
    fn drop(&mut self) {
        self.completion.cancelled.store(true, Ordering::Release);
    }
}
struct Work {
    backup: Backup,
    max_bytes: usize,
    latest_start: Instant,
    guard: Guard,
}
// Publish a terminal uncertain failure and release admission even if an adapter panics.
struct Guard {
    completion: Arc<Completion>,
    busy: Arc<AtomicBool>,
    done: bool,
}
impl Guard {
    fn finish(mut self, result: Result) {
        self.done = true;
        // Release admission before waking the caller, so a following save can start.
        self.busy.store(false, Ordering::Release);
        self.completion.complete(result);
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if !self.done {
            self.busy.store(false, Ordering::Release);
            self.completion
                .complete(Err(executor::failure(BackupError::Unavailable, true)));
        }
    }
}
/// Only one admitted request including abandoned in-flight work. A dropped save
/// future cannot create an unbounded queue of detached filesystem operations.
pub struct NativePlaylistBackups {
    tx: Option<SyncSender<Work>>,
    busy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl NativePlaylistBackups {
    pub fn open(root: PathBuf) -> std::result::Result<Self, BackupFailure> {
        Self::with_factory(move || NativeStore::open_playlist_backups(root))
    }
    /// Factory and returned store are used exclusively on the worker. The store
    /// need not be Send; this also permits deterministic blocking test adapters.
    pub fn with_factory<
        S: AtomicStore,
        F: FnOnce() -> std::result::Result<S, crate::Error> + Send + 'static,
    >(
        factory: F,
    ) -> std::result::Result<Self, BackupFailure> {
        let (tx, rx) = mpsc::sync_channel::<Work>(1);
        let busy = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("playlist-backups".into())
            .spawn(move || {
                let mut factory = Some(factory);
                let mut repository = None;
                while let Ok(work) = rx.recv() {
                    let completion = Arc::clone(&work.guard.completion);
                    let guard = work.guard;
                    let can_start = || {
                        if worker_stop.load(Ordering::Acquire)
                            || completion.cancelled.load(Ordering::Acquire)
                        {
                            Err(executor::failure(
                                BackupError::Context(DomainError::Cancelled),
                                false,
                            ))
                        } else if Instant::now() > work.latest_start {
                            Err(executor::failure(
                                BackupError::Context(DomainError::Deadline),
                                false,
                            ))
                        } else {
                            Ok(())
                        }
                    };
                    let result = (|| {
                        can_start()?;
                        if let Some(factory) = factory.take() {
                            repository = Some(
                                factory()
                                    .map(Repository::new)
                                    .map_err(|e| executor::map_error(e.into())),
                            );
                        }
                        match repository.as_mut() {
                            Some(Ok(repository)) => {
                                executor::save(repository, &work.backup, work.max_bytes, can_start)
                            }
                            Some(Err(error)) => Err(*error),
                            // Only reachable if the internal factory ownership invariant fails.
                            None => Err(executor::failure(BackupError::Unavailable, false)),
                        }
                    })();
                    guard.finish(result);
                }
            })
            .map_err(|_| executor::failure(BackupError::Unavailable, false))?;
        Ok(Self {
            tx: Some(tx),
            busy,
            stop,
            join: Some(join),
        })
    }
    pub fn is_idle(&self) -> bool {
        !self.busy.load(Ordering::Acquire)
    }
    pub fn begin_shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.tx.take();
    }
    /// Never waits. Drop requests shutdown and detaches in-flight OS work.
    pub fn try_join(&mut self) -> Option<std::result::Result<(), BackupFailure>> {
        if !self.join.as_ref()?.is_finished() {
            return None;
        }
        let join = self.join.take()?;
        self.tx.take();
        Some(
            join.join()
                .map_err(|_| executor::failure(BackupError::Unavailable, true)),
        )
    }
}
impl PlaylistBackupStore for NativePlaylistBackups {
    fn save<'a>(
        &'a mut self,
        backup: PlaylistBackup,
        clock: &'a dyn Clock,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, std::result::Result<BackupReceipt, BackupFailure>> {
        Box::pin(async move {
            let backup = executor::prepare(backup, clock, context)?;
            let tx = self
                .tx
                .as_ref()
                .ok_or_else(|| executor::failure(BackupError::Unavailable, false))?;
            let remaining = context.correlation.deadline.0.saturating_sub(clock.now().0);
            let latest_start = Instant::now()
                .checked_add(Duration::from_millis(remaining))
                .ok_or_else(|| executor::failure(BackupError::Exhausted, false))?;
            if self
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(executor::failure(BackupError::Capacity, false));
            }
            let completion = Arc::new(Completion::default());
            let work = Work {
                backup,
                max_bytes: context.limits.request_bytes,
                latest_start,
                guard: Guard {
                    completion: Arc::clone(&completion),
                    busy: Arc::clone(&self.busy),
                    done: false,
                },
            };
            if let Err(error) = tx.try_send(work) {
                self.busy.store(false, Ordering::Release);
                return Err(executor::failure(
                    match error {
                        TrySendError::Full(_) => BackupError::Capacity,
                        TrySendError::Disconnected(_) => BackupError::Unavailable,
                    },
                    false,
                ));
            }
            let result = Wait {
                completion,
                clock,
                context,
            }
            .await;
            executor::finish(result, clock, context)
        })
    }
}
impl Drop for NativePlaylistBackups {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}
