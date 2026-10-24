//! Bounded P01 control routing experiment, not a production playback/queue owner.
use crate::{AudioOutput, Error, Mpv, NativePlaybackStatus};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
};

pub const CONTROL_CAPACITY: usize = 8;
const MAX_BUDGET: Duration = Duration::from_secs(3);

/// Local files only in this experiment. Network/authenticated sources are unsupported.
pub enum ControlCommand {
    LoadLocal(PathBuf),
    Pause(bool),
    SeekSeconds(f64),
    Position,
    Paused,
    PlaybackStatus,
    TogglePause,
    SeekRelativeSeconds(f64),
    SeekObservedSeconds(f64),
    VolumePercent,
    SetVolumePercent(f64),
}
#[derive(Debug)]
pub enum ControlValue {
    Acknowledged,
    Position(Option<f64>),
    Paused(bool),
    PlaybackStatus(NativePlaybackStatus),
    SeekObserved(Option<f64>),
    VolumePercent(f64),
}
#[derive(Debug)]
pub enum ControlFailure {
    QueueFull,
    Stale,
    Deadline,
    Cancelled,
    Stopped,
    Engine(Error),
}
#[derive(Debug)]
pub struct ControlReply {
    pub generation: u64,
    pub request_id: u64,
    pub result: Result<ControlValue, ControlFailure>,
}
struct Envelope {
    generation: u64,
    request_id: u64,
    deadline: Instant,
    command: ControlCommand,
    cancelled: Arc<AtomicBool>,
    reply: SyncSender<ControlReply>,
}

/// A one-slot completion. Dropping it cancels queued work; an issued IPC mutation
/// may already have executed and cannot be undone by cancellation or timeout.
pub struct ControlRequest {
    receive: Receiver<ControlReply>,
    cancelled: Arc<AtomicBool>,
}
impl ControlRequest {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn try_reply(&self) -> Result<Option<ControlReply>, ControlFailure> {
        match self.receive.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ControlFailure::Stopped),
        }
    }
    /// Blocking test/CLI API. UI code must poll `try_reply` instead.
    pub fn wait(&self, timeout: Duration) -> Result<ControlReply, ControlFailure> {
        self.receive
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => ControlFailure::Deadline,
                mpsc::RecvTimeoutError::Disconnected => ControlFailure::Stopped,
            })
    }
}
impl Drop for ControlRequest {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
#[derive(Clone)]
pub struct ControlClient {
    send: SyncSender<Envelope>,
    stopped: Arc<AtomicBool>,
}
impl ControlClient {
    pub fn submit(
        &self,
        generation: u64,
        request_id: u64,
        budget: Duration,
        command: ControlCommand,
    ) -> Result<ControlRequest, ControlFailure> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(ControlFailure::Stopped);
        }
        if budget.is_zero() {
            return Err(ControlFailure::Deadline);
        }
        if matches!(command, ControlCommand::SeekSeconds(seconds) if !seconds.is_finite() || seconds < 0.0)
        {
            return Err(ControlFailure::Engine(Error::InvalidInput));
        }
        if matches!(command, ControlCommand::SeekRelativeSeconds(seconds) if !seconds.is_finite()) {
            return Err(ControlFailure::Engine(Error::InvalidInput));
        }
        // Bound ingress payload memory and reject values IPC cannot encode.
        if let ControlCommand::LoadLocal(path) = &command
            && path
                .to_str()
                .is_none_or(|path| path.is_empty() || path.len() > 4096 || path.contains('\0'))
        {
            return Err(ControlFailure::Engine(Error::InvalidInput));
        }
        let (reply, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let envelope = Envelope {
            generation,
            request_id,
            deadline: Instant::now() + budget.min(MAX_BUDGET),
            command,
            cancelled: cancelled.clone(),
            reply,
        };
        self.send.try_send(envelope).map_err(|error| match error {
            TrySendError::Full(_) => ControlFailure::QueueFull,
            TrySendError::Disconnected(_) => ControlFailure::Stopped,
        })?;
        Ok(ControlRequest { receive, cancelled })
    }
}

/// One worker creates and exclusively owns one mpv. Construction and Drop never
/// wait on IPC. Shutdown bypasses ingress and cannot be starved by queued work.
/// Existing startup can take up to two three-second exchanges; shutdown during
/// startup therefore completes within that startup bound plus 600ms cleanup.
/// Running commands are capped at three seconds plus cleanup (OS scheduling and
/// process kill/reap latency are external to these protocol budgets).
pub struct ControlSession {
    client: ControlClient,
    done: Receiver<Result<(), Error>>,
}
impl ControlSession {
    pub fn start(binary: PathBuf, generation: u64, output: AudioOutput) -> Result<Self, Error> {
        let (send, receive) = mpsc::sync_channel::<Envelope>(CONTROL_CAPACITY);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let (done_send, done) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("p01-controls".into())
            .spawn(move || {
                let result = (|| {
                    let mut mpv = match Mpv::start(&binary, generation, output) {
                        Ok(mpv) => mpv,
                        Err(error) => {
                            for request in receive.try_iter() {
                                complete(request, Err(ControlFailure::Stopped));
                            }
                            return Err(error);
                        }
                    };
                    while !stop.load(Ordering::Acquire) {
                        let request = match receive.recv_timeout(Duration::from_millis(10)) {
                            Ok(request) => request,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        let result = if stop.load(Ordering::Acquire) {
                            Err(ControlFailure::Stopped)
                        } else if request.generation != generation {
                            Err(ControlFailure::Stale)
                        } else if request.cancelled.load(Ordering::Acquire) {
                            Err(ControlFailure::Cancelled)
                        } else if Instant::now() >= request.deadline {
                            Err(ControlFailure::Deadline)
                        } else {
                            let result = match &request.command {
                                ControlCommand::LoadLocal(path) => mpv
                                    .load_local_until(path, false, request.deadline)
                                    .map(|()| ControlValue::Acknowledged),
                                ControlCommand::Pause(paused) => mpv
                                    .pause_until(*paused, request.deadline)
                                    .map(|()| ControlValue::Acknowledged),
                                ControlCommand::SeekSeconds(seconds) => mpv
                                    .seek_until(*seconds, request.deadline)
                                    .map(|()| ControlValue::Acknowledged),
                                ControlCommand::PlaybackStatus => mpv
                                    .playback_status_until(request.deadline)
                                    .map(ControlValue::PlaybackStatus),
                                ControlCommand::Paused => {
                                    mpv.paused_until(request.deadline).map(ControlValue::Paused)
                                }
                                ControlCommand::TogglePause => mpv
                                    .paused_until(request.deadline)
                                    .and_then(|paused| mpv.pause_until(!paused, request.deadline))
                                    .map(|()| ControlValue::Acknowledged),
                                ControlCommand::SeekRelativeSeconds(offset) => mpv
                                    .seek_observed_until(*offset, true, request.deadline)
                                    .map(ControlValue::SeekObserved),
                                ControlCommand::SeekObservedSeconds(seconds) => mpv
                                    .seek_observed_until(*seconds, false, request.deadline)
                                    .map(ControlValue::SeekObserved),
                                ControlCommand::VolumePercent => mpv
                                    .volume_percent_until(request.deadline)
                                    .map(ControlValue::VolumePercent),
                                ControlCommand::SetVolumePercent(percent) => mpv
                                    .set_volume_percent_until(*percent, request.deadline)
                                    .map(|()| ControlValue::Acknowledged),
                                ControlCommand::Position => mpv
                                    .probe_position(
                                        request.deadline.saturating_duration_since(Instant::now()),
                                    )
                                    .map(ControlValue::Position),
                            };
                            if stop.load(Ordering::Acquire) {
                                Err(ControlFailure::Stopped)
                            } else if request.cancelled.load(Ordering::Acquire) {
                                Err(ControlFailure::Cancelled)
                            } else if Instant::now() >= request.deadline {
                                Err(ControlFailure::Deadline)
                            } else {
                                result.map_err(ControlFailure::Engine)
                            }
                        };
                        let engine_failed = !mpv.accepts_generation(generation);
                        complete(request, result);
                        if engine_failed {
                            break;
                        }
                    }
                    stop.store(true, Ordering::Release);
                    for request in receive.try_iter() {
                        complete(request, Err(ControlFailure::Stopped));
                    }
                    mpv.shutdown()
                })();
                stop.store(true, Ordering::Release);
                let _ = done_send.try_send(result);
            })?;
        Ok(Self {
            client: ControlClient { send, stopped },
            done,
        })
    }
    pub fn client(&self) -> ControlClient {
        self.client.clone()
    }
    pub fn shutdown(&self) {
        self.client.stopped.store(true, Ordering::Release);
    }
    /// Explicit blocking cleanup for CLI/tests; timeout leaves worker owning cleanup.
    pub fn wait_shutdown(&self, timeout: Duration) -> Result<(), ControlFailure> {
        self.done
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => ControlFailure::Deadline,
                mpsc::RecvTimeoutError::Disconnected => ControlFailure::Stopped,
            })?
            .map_err(ControlFailure::Engine)
    }
}
impl Drop for ControlSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn complete(request: Envelope, result: Result<ControlValue, ControlFailure>) {
    let _ = request.reply.try_send(ControlReply {
        generation: request.generation,
        request_id: request.request_id,
        result,
    });
}
