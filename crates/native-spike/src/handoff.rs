//! Bounded managed-engine replacement experiment. This does not select a fallback engine.
use crate::{
    AudioOutput, ControlClient, ControlCommand, ControlFailure, ControlSession, ControlValue,
    Error, NativePlaybackStatus,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
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
    /// `replace` installs only an endpoint; `replace_prepared` first verifies
    /// local position and playback status. Neither certifies device output.
    pub result: Result<ControlClient, HandoffFailure>,
}

/// Local-only state restored before a prepared endpoint is published.
#[derive(Debug)]
pub struct LocalPreparation {
    pub source: PathBuf,
    pub position_seconds: f64,
    pub paused: bool,
    pub volume_percent: f64,
}

struct Replacement {
    preparation: Option<LocalPreparation>,
    deadline: Option<Instant>,
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
/// Old endpoint clones are stopped before a new worker can start. No queue is
/// restored and no automatic fallback is tried. `replace` installs only
/// an endpoint; `replace_prepared` verifies local source/settings restoration.
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
        self.admit(binary, generation, output, None, None)
    }

    /// Restore a local source and verify observed position/status before publishing.
    /// The admission deadline includes queueing, retirement and startup; cleanup
    /// retains its separate safety budget even after this deadline expires.
    pub fn replace_prepared(
        &self,
        binary: PathBuf,
        generation: u64,
        output: AudioOutput,
        preparation: LocalPreparation,
        budget: Duration,
    ) -> Result<HandoffRequest, HandoffFailure> {
        if !valid_path(&preparation.source)
            || !preparation.position_seconds.is_finite()
            || preparation.position_seconds < 0.0
            || !preparation.volume_percent.is_finite()
            || !(0.0..=100.0).contains(&preparation.volume_percent)
        {
            return Err(HandoffFailure::Engine(Error::InvalidInput));
        }
        if budget.is_zero() {
            return Err(HandoffFailure::Deadline);
        }
        let deadline = Instant::now() + budget.min(Duration::from_secs(15));
        self.admit(
            binary,
            generation,
            output,
            Some(preparation),
            Some(deadline),
        )
    }

    fn admit(
        &self,
        binary: PathBuf,
        generation: u64,
        output: AudioOutput,
        preparation: Option<LocalPreparation>,
        deadline: Option<Instant>,
    ) -> Result<HandoffRequest, HandoffFailure> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(HandoffFailure::Stopped);
        }
        if !valid_path(&binary) {
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
            preparation,
            deadline,
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
        if request.preparation.is_some()
            && let Err(failure) = prepare(&session.client(), &request, stop)
        {
            if let Err(error) = retire(session) {
                stop.store(true, Ordering::Release);
                complete(request, Err(error));
                return Err(HandoffFailure::Stopped);
            }
            complete(request, Err(failure));
            continue;
        }
        // Final pre-publication check; completion may win a concurrent cancellation.
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
            stop.store(true, Ordering::Release);
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
    } else if request
        .deadline
        .is_some_and(|deadline| Instant::now() >= deadline)
    {
        Some(HandoffFailure::Deadline)
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

fn valid_path(path: &std::path::Path) -> bool {
    path.to_str()
        .is_some_and(|path| !path.is_empty() && path.len() <= 4096 && !path.contains('\0'))
}

fn control_failure(failure: ControlFailure) -> HandoffFailure {
    match failure {
        ControlFailure::Engine(error) => HandoffFailure::Engine(error),
        ControlFailure::Deadline => HandoffFailure::Deadline,
        ControlFailure::Cancelled => HandoffFailure::Cancelled,
        ControlFailure::Stopped => HandoffFailure::Stopped,
        ControlFailure::QueueFull | ControlFailure::Stale => {
            HandoffFailure::Engine(Error::Protocol)
        }
    }
}

fn prepare(
    client: &ControlClient,
    request: &Replacement,
    stop: &AtomicBool,
) -> Result<(), HandoffFailure> {
    let preparation = request
        .preparation
        .as_ref()
        .ok_or(HandoffFailure::Engine(Error::Protocol))?;
    let mut request_id = 0;
    let mut command = |command| {
        request_id += 1;
        prepared_command(client, request, stop, request_id, command)
    };
    acknowledged(command(ControlCommand::Pause(true))?)?;
    acknowledged(command(ControlCommand::LoadLocal(
        preparation.source.clone(),
    ))?)?;
    observe_position(&mut command, 0.0)?;
    match command(ControlCommand::SeekObservedSeconds(
        preparation.position_seconds,
    ))? {
        ControlValue::SeekObserved(Some(position))
            if position.is_finite() && (position - preparation.position_seconds).abs() <= 0.1 => {}
        ControlValue::SeekObserved(None) => return Err(HandoffFailure::Engine(Error::Unsupported)),
        _ => return Err(HandoffFailure::Engine(Error::Protocol)),
    }
    observe_position(&mut command, preparation.position_seconds)?;
    acknowledged(command(ControlCommand::SetVolumePercent(
        preparation.volume_percent,
    ))?)?;
    match command(ControlCommand::VolumePercent)? {
        ControlValue::VolumePercent(volume)
            if volume.is_finite() && (volume - preparation.volume_percent).abs() <= 0.001 => {}
        _ => return Err(HandoffFailure::Engine(Error::Protocol)),
    }
    acknowledged(command(ControlCommand::Pause(preparation.paused))?)?;
    let expected = if preparation.paused {
        NativePlaybackStatus::Paused
    } else {
        NativePlaybackStatus::Playing
    };
    match command(ControlCommand::PlaybackStatus)? {
        ControlValue::PlaybackStatus(status) if status == expected => Ok(()),
        _ => Err(HandoffFailure::Engine(Error::Protocol)),
    }
}

fn acknowledged(value: ControlValue) -> Result<(), HandoffFailure> {
    match value {
        ControlValue::Acknowledged => Ok(()),
        _ => Err(HandoffFailure::Engine(Error::Protocol)),
    }
}

fn observe_position(
    command: &mut impl FnMut(ControlCommand) -> Result<ControlValue, HandoffFailure>,
    target: f64,
) -> Result<(), HandoffFailure> {
    loop {
        match command(ControlCommand::Position)? {
            ControlValue::Position(Some(position))
                if position.is_finite() && (position - target).abs() <= 0.1 =>
            {
                return Ok(());
            }
            ControlValue::Position(_) => std::thread::sleep(Duration::from_millis(5)),
            _ => return Err(HandoffFailure::Engine(Error::Protocol)),
        }
    }
}

fn prepared_command(
    client: &ControlClient,
    request: &Replacement,
    stop: &AtomicBool,
    request_id: u64,
    command: ControlCommand,
) -> Result<ControlValue, HandoffFailure> {
    if let Some(failure) = interrupted(request, stop) {
        return Err(failure);
    }
    let deadline = request
        .deadline
        .ok_or(HandoffFailure::Engine(Error::Protocol))?;
    let pending = client
        .submit(
            request.generation,
            request_id,
            deadline.saturating_duration_since(Instant::now()),
            command,
        )
        .map_err(control_failure)?;
    loop {
        if let Some(failure) = interrupted(request, stop) {
            return Err(failure);
        }
        if let Some(reply) = pending.try_reply().map_err(control_failure)? {
            if reply.generation != request.generation || reply.request_id != request_id {
                return Err(HandoffFailure::Engine(Error::Protocol));
            }
            return reply.result.map_err(control_failure);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
