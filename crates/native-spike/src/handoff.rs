//! Bounded same-mpv replacement experiment. This does not select a fallback engine.
use crate::{AudioOutput, ControlClient, ControlFailure, ControlSession, Error};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::Duration,
};

pub const HANDOFF_CAPACITY: usize = 1;
// ControlSession startup has two <=3s exchanges and <=600ms cleanup. This
// includes margin; OS scheduling/kill/reap latency is not a hard realtime bound.
const CLEANUP_BUDGET: Duration = Duration::from_secs(7);

#[derive(Debug)]
pub enum HandoffFailure {
    QueueFull,
    Stale,
    Cancelled,
    Stopped,
    Deadline,
    Engine(Error),
}

pub struct HandoffReply {
    pub generation: u64,
    /// Success installs an endpoint, not an engine/source readiness guarantee.
    /// Startup failure is reported by the endpoint's commands/worker cleanup.
    pub result: Result<ControlClient, HandoffFailure>,
}

struct Replacement {
    binary: PathBuf,
    generation: u64,
    output: AudioOutput,
    cancelled: Arc<AtomicBool>,
    reply: SyncSender<HandoffReply>,
}

/// A one-slot completion. Cancellation can preserve the old endpoint only before
/// retirement begins. It cannot undo a replacement whose reply was delivered.
pub struct HandoffRequest {
    receive: Receiver<HandoffReply>,
    cancelled: Arc<AtomicBool>,
}
impl HandoffRequest {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn try_reply(&self) -> Result<Option<HandoffReply>, HandoffFailure> {
        match self.receive.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(HandoffFailure::Stopped),
        }
    }
    /// Blocking CLI/test API. UI/event executors must poll `try_reply` instead.
    /// A wait timeout does not cancel the request; use `cancel` or drop it.
    pub fn wait(&self, timeout: Duration) -> Result<HandoffReply, HandoffFailure> {
        self.receive.recv_timeout(timeout).map_err(wait_failure)
    }
}
impl Drop for HandoffRequest {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// One supervisor serializes retiring and replacing managed control sessions.
/// Construction, replacement admission and Drop do not wait for IPC or cleanup.
/// Old endpoint clones are stopped before a new worker can start. No source,
/// position, queue or settings are restored, and no automatic fallback is tried.
///
/// A successful admission consumes its generation, even if later cancelled or
/// failed. Generations must strictly increase; queue-full/invalid submissions do
/// not consume them. Each teardown waits at most seven seconds for confirmation.
/// Any teardown failure permanently stops admission rather than risking overlap.
/// A timed-out teardown can still be cleaning up in its owning ControlSession
/// worker; this supervisor never starts another engine after that failure.
pub struct HandoffSession {
    send: SyncSender<Replacement>,
    highest: Mutex<Option<u64>>,
    stopped: Arc<AtomicBool>,
    done: Receiver<Result<(), HandoffFailure>>,
}
impl HandoffSession {
    pub fn start() -> Result<Self, Error> {
        let (send, receive) = mpsc::sync_channel(HANDOFF_CAPACITY);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let (done_send, done) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("p01-handoff".into())
            .spawn(move || {
                let result = supervise(&receive, &stop);
                stop.store(true, Ordering::Release);
                for request in receive.try_iter() {
                    complete(request, Err(HandoffFailure::Stopped));
                }
                let _ = done_send.try_send(result);
            })?;
        Ok(Self {
            send,
            highest: Mutex::new(None),
            stopped,
            done,
        })
    }

    pub fn replace(
        &self,
        binary: PathBuf,
        generation: u64,
        output: AudioOutput,
    ) -> Result<HandoffRequest, HandoffFailure> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(HandoffFailure::Stopped);
        }
        if binary
            .to_str()
            .is_none_or(|path| path.is_empty() || path.len() > 4096 || path.contains('\0'))
        {
            return Err(HandoffFailure::Engine(Error::InvalidInput));
        }
        // Keep admission order and generation reservation atomic without waiting
        // behind concurrent submitters. This lock never covers IPC or cleanup.
        let mut highest = match self.highest.try_lock() {
            Ok(highest) => highest,
            Err(TryLockError::WouldBlock) => return Err(HandoffFailure::QueueFull),
            Err(TryLockError::Poisoned(_)) => {
                self.shutdown();
                return Err(HandoffFailure::Stopped);
            }
        };
        if highest.is_some_and(|highest| generation <= highest) {
            return Err(HandoffFailure::Stale);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = Replacement {
            binary,
            generation,
            output,
            cancelled: cancelled.clone(),
            reply,
        };
        self.send.try_send(request).map_err(|error| match error {
            TrySendError::Full(_) => HandoffFailure::QueueFull,
            TrySendError::Disconnected(_) => HandoffFailure::Stopped,
        })?;
        *highest = Some(generation);
        Ok(HandoffRequest { receive, cancelled })
    }

    /// Shutdown bypasses the replacement queue; an in-progress teardown retains
    /// its cleanup budget. No new replacement starts after observing this flag.
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
    }
    /// Blocking CLI/test cleanup wait. A timeout leaves workers owning cleanup.
    /// After a replacement cleanup failure, its reply carries the cause and this
    /// wait reports Stopped. The result is a one-shot completion.
    pub fn wait_shutdown(&self, timeout: Duration) -> Result<(), HandoffFailure> {
        self.done.recv_timeout(timeout).map_err(wait_failure)?
    }
}
impl Drop for HandoffSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn supervise(receive: &Receiver<Replacement>, stop: &AtomicBool) -> Result<(), HandoffFailure> {
    let mut active: Option<ControlSession> = None;
    while !stop.load(Ordering::Acquire) {
        let request = match receive.recv_timeout(Duration::from_millis(10)) {
            Ok(request) => request,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if let Some(failure) = interrupted(&request, stop) {
            complete(request, Err(failure));
            continue;
        }
        if let Some(previous) = active.take()
            && let Err(error) = retire(previous)
        {
            stop.store(true, Ordering::Release);
            complete(request, Err(error));
            return Err(HandoffFailure::Stopped);
        }
        if let Some(failure) = interrupted(&request, stop) {
            complete(request, Err(failure));
            continue;
        }
        let session =
            match ControlSession::start(request.binary.clone(), request.generation, request.output)
            {
                Ok(session) => session,
                Err(error) => {
                    complete(request, Err(HandoffFailure::Engine(error)));
                    continue;
                }
            };
        if let Some(failure) = interrupted(&request, stop) {
            if let Err(error) = retire(session) {
                stop.store(true, Ordering::Release);
                complete(request, Err(error));
                return Err(HandoffFailure::Stopped);
            }
            complete(request, Err(failure));
            continue;
        }
        let client = session.client();
        if complete(request, Ok(client)) {
            active = Some(session);
        } else if let Err(error) = retire(session) {
            return Err(error);
        }
    }
    if let Some(session) = active {
        retire(session)?;
    }
    Ok(())
}
fn interrupted(request: &Replacement, stop: &AtomicBool) -> Option<HandoffFailure> {
    if stop.load(Ordering::Acquire) {
        Some(HandoffFailure::Stopped)
    } else if request.cancelled.load(Ordering::Acquire) {
        Some(HandoffFailure::Cancelled)
    } else {
        None
    }
}
fn retire(session: ControlSession) -> Result<(), HandoffFailure> {
    session.shutdown();
    session
        .wait_shutdown(CLEANUP_BUDGET)
        .map_err(|error| match error {
            ControlFailure::Deadline => HandoffFailure::Deadline,
            ControlFailure::Engine(error) => HandoffFailure::Engine(error),
            _ => HandoffFailure::Stopped,
        })
}
fn complete(request: Replacement, result: Result<ControlClient, HandoffFailure>) -> bool {
    request
        .reply
        .try_send(HandoffReply {
            generation: request.generation,
            result,
        })
        .is_ok()
}
fn wait_failure(error: mpsc::RecvTimeoutError) -> HandoffFailure {
    match error {
        mpsc::RecvTimeoutError::Timeout => HandoffFailure::Deadline,
        mpsc::RecvTimeoutError::Disconnected => HandoffFailure::Stopped,
    }
}
