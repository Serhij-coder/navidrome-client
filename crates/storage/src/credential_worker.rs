//! Native bounded worker. A provider prompt may block indefinitely; Drop never joins.
use crate::{
    credential_executor::{
        Command, DispatchError, Dispatcher, Generation, Reply, Request, RequestId,
    },
    credentials::{CredentialError, NoPersistence, PersistentCredentials},
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
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
    InvalidCapacity,
    Full,
    Closed,
    Exhausted,
    Invalid(DispatchError),
    Factory(CredentialError),
    Panicked,
    SpawnFailed,
}
/// A disconnected worker without a reply leaves any started mutation uncertain.
pub struct CredentialWorker {
    generation: Generation,
    start: Instant,
    tx: Option<SyncSender<Work>>,
    rx: Receiver<Reply>,
    pending: BTreeMap<u64, Arc<AtomicBool>>,
    next: u64,
    capacity: usize,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), CredentialError>>>,
}
impl CredentialWorker {
    /// Start a session-only worker; construction and session operations do no provider I/O.
    pub fn new(generation: Generation, capacity: usize) -> Result<Self, WorkerError> {
        Self::with_factory(generation, capacity, || Ok(NoPersistence))
    }
    /// The factory runs on the worker; its returned provider need not be Send.
    pub fn with_factory<
        B: PersistentCredentials,
        F: FnOnce() -> Result<B, CredentialError> + Send + 'static,
    >(
        generation: Generation,
        capacity: usize,
        factory: F,
    ) -> Result<Self, WorkerError> {
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
                let mut dispatcher = Dispatcher::with_backend(generation, factory()?);
                while let Ok(work) = work_rx.recv() {
                    let cancelled = worker_stop.load(Ordering::Acquire)
                        || work.cancelled.load(Ordering::Acquire);
                    let reply = dispatcher.dispatch(work.request, start.elapsed(), cancelled);
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
    /// Admit at most `capacity` requests, including replies not yet consumed by `poll`.
    /// A zero wait, or reaching the latest-start time exactly, prevents operation start.
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
        let latest_start = self
            .start
            .elapsed()
            .checked_add(queue_wait)
            .ok_or(WorkerError::Exhausted)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let work = Work {
            request: Request {
                generation: self.generation,
                id,
                latest_start,
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
    /// Request cancellation before start. `true` acknowledges the request, not cancellation
    /// of an already started operation; its real result is still delivered.
    pub fn cancel(&self, generation: Generation, id: RequestId) -> bool {
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
    /// Consume a reply and release its capacity slot. Closed or a panic without a reply
    /// leaves started mutations uncertain; reconcile before retrying.
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
    /// Stop admission and request cancellation of queued work without waiting.
    pub fn begin_shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.tx.take();
    }
    /// Join only after the worker finishes; a provider prompt can wait indefinitely.
    /// Drop requests shutdown and detaches rather than joining.
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
impl Drop for CredentialWorker {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}
