//! Dedicated bounded blocking worker. Shutdown cannot interrupt an in-flight OS call.
use crate::{
    AtomicStore, Error, NativeStore,
    session::{Command, Reply, Request, RequestId, Session, SessionError, SessionGeneration},
    state::StateRepository,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
struct Work {
    request: Request,
    cancelled: Arc<AtomicBool>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerError {
    Full,
    InvalidCapacity,
    Closed,
    Exhausted,
    Invalid(SessionError),
    Factory(Error),
    Panicked,
    SpawnFailed,
}
pub struct NativeWorker {
    generation: SessionGeneration,
    start: Instant,
    tx: Option<SyncSender<Work>>,
    rx: Receiver<Reply>,
    pending: BTreeMap<u64, Arc<AtomicBool>>,
    next: u64,
    capacity: usize,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), Error>>>,
}
impl NativeWorker {
    pub fn open(
        generation: SessionGeneration,
        root: PathBuf,
        capacity: usize,
    ) -> Result<Self, WorkerError> {
        Self::with_factory(generation, capacity, move || {
            Ok((
                NativeStore::open_state(&root)?,
                NativeStore::open_imports(&root)?,
                NativeStore::open(&root)?,
            ))
        })
    }
    pub fn with_factory<A, J, P, F>(
        generation: SessionGeneration,
        capacity: usize,
        factory: F,
    ) -> Result<Self, WorkerError>
    where
        A: AtomicStore,
        J: AtomicStore,
        P: AtomicStore,
        F: FnOnce() -> Result<(A, J, P), Error> + Send + 'static,
    {
        if !(1..=8).contains(&capacity) {
            return Err(WorkerError::InvalidCapacity);
        }
        let (tx, work_rx) = mpsc::sync_channel::<Work>(capacity);
        let (reply_tx, rx) = mpsc::sync_channel(capacity);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let start = Instant::now();
        let join = thread::Builder::new()
            .spawn(move || {
                let (a, j, p) = factory()?;
                let mut session = Session::new(generation, StateRepository::new(a, j, p));
                while let Ok(work) = work_rx.recv() {
                    let cancelled = worker_stop.load(Ordering::Acquire)
                        || work.cancelled.load(Ordering::Acquire);
                    let reply = session.dispatch(work.request, start.elapsed(), cancelled);
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
                Ok(())
            })
            .map_err(|_| WorkerError::SpawnFailed)?;
        Ok(Self {
            generation,
            start,
            tx: Some(tx),
            rx,
            pending: BTreeMap::new(),
            next: 0,
            capacity,
            stop,
            join: Some(join),
        })
    }
    pub fn submit(
        &mut self,
        command: Command,
        queue_wait: Duration,
    ) -> Result<RequestId, WorkerError> {
        command.validate_size().map_err(WorkerError::Invalid)?;
        if self.join.as_ref().is_some_and(JoinHandle::is_finished) {
            self.tx.take();
        }
        if self.tx.is_none() || self.stop.load(Ordering::Acquire) {
            return Err(WorkerError::Closed);
        }
        if self.pending.len() >= self.capacity {
            return Err(WorkerError::Full);
        }
        let n = self.next.checked_add(1).ok_or(WorkerError::Exhausted)?;
        let id = RequestId(NonZeroU64::new(n).ok_or(WorkerError::Exhausted)?);
        let deadline = self
            .start
            .elapsed()
            .checked_add(queue_wait)
            .ok_or(WorkerError::Exhausted)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let work = Work {
            request: Request {
                generation: self.generation,
                id,
                deadline,
                command,
            },
            cancelled: Arc::clone(&cancelled),
        };
        match self.tx.as_ref().ok_or(WorkerError::Closed)?.try_send(work) {
            Ok(()) => {
                self.next = n;
                self.pending.insert(n, cancelled);
                Ok(id)
            }
            Err(TrySendError::Full(_)) => Err(WorkerError::Full),
            Err(TrySendError::Disconnected(_)) => Err(WorkerError::Closed),
        }
    }
    pub fn cancel(&self, generation: SessionGeneration, id: RequestId) -> bool {
        if generation != self.generation {
            return false;
        }
        if let Some(flag) = self.pending.get(&id.0.get()) {
            flag.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }
    pub fn poll(&mut self) -> Result<Option<Reply>, WorkerError> {
        match self.rx.try_recv() {
            Ok(reply) => {
                self.pending.remove(&reply.id.0.get());
                Ok(Some(reply))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.tx.take();
                Err(WorkerError::Closed)
            }
        }
    }
    pub fn begin_shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.tx.take();
    }
    pub fn try_join(&mut self) -> Option<Result<(), WorkerError>> {
        if !self.join.as_ref()?.is_finished() {
            return None;
        }
        let join = self.join.take()?;
        self.tx.take();
        Some(match join.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(WorkerError::Factory(e)),
            Err(_) => Err(WorkerError::Panicked),
        })
    }
}
impl Drop for NativeWorker {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}
