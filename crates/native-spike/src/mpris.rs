//! Opt-in P01 private Unix bus experiment for one prepared local source.
//! Caller owns the bus and must not load another source while this service lives.
//! No host session lookup, queue, production metadata, URI loading or media keys.
use crate::{ControlClient, ControlCommand, ControlValue};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use zbus::{
    fdo,
    zvariant::{ObjectPath, OwnedValue},
};

pub const MPRIS_TRACK_PATH: &str = "/org/feishin/P01/OwnedSource";
const PATH: &str = "/org/mpris/MediaPlayer2";
const LIMIT: usize = 8;
const BUDGET: Duration = Duration::from_secs(2);

pub fn micros_to_seconds(micros: i64) -> Result<f64, crate::Error> {
    if micros < 0 {
        return Err(crate::Error::InvalidInput);
    }
    Ok(micros as f64 / 1_000_000.0)
}
pub fn seconds_to_micros(seconds: f64) -> Result<i64, crate::Error> {
    let micros = seconds * 1_000_000.0;
    if !seconds.is_finite() || seconds < 0.0 || micros >= i64::MAX as f64 {
        return Err(crate::Error::InvalidInput);
    }
    Ok(micros.round() as i64)
}
fn failed() -> fdo::Error {
    fdo::Error::Failed("P01 control failed or expired".into())
}
fn unsupported() -> fdo::Error {
    fdo::Error::NotSupported("Unsupported by single-source P01 probe".into())
}

struct Admission(Arc<AtomicUsize>);
impl Drop for Admission {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
#[derive(Clone)]
struct Player {
    client: ControlClient,
    generation: u64,
    next: Arc<AtomicU64>,
    pending: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
}
struct Call {
    _admission: Admission,
    deadline: Instant,
}
async fn emit_until<T>(
    call: &Call,
    future: impl std::future::Future<Output = zbus::Result<T>>,
) -> fdo::Result<T> {
    use std::{future::Future, task::Poll};
    let mut future = std::pin::pin!(future);
    let mut timer = std::pin::pin!(async_io::Timer::at(call.deadline));
    std::future::poll_fn(|cx| {
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(failed()));
        }
        future
            .as_mut()
            .poll(cx)
            .map(|result| result.map_err(|_| failed()))
    })
    .await
}
impl Player {
    fn call(&self) -> fdo::Result<Call> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(failed());
        }
        self.pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < LIMIT).then_some(count + 1)
            })
            .map_err(|_| fdo::Error::LimitsExceeded("P01 pending calls full".into()))?;
        Ok(Call {
            _admission: Admission(self.pending.clone()),
            deadline: Instant::now() + BUDGET,
        })
    }
    async fn request(&self, call: &Call, command: ControlCommand) -> fdo::Result<ControlValue> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let request = self
            .client
            .submit(
                self.generation,
                id,
                call.deadline.saturating_duration_since(Instant::now()),
                command,
            )
            .map_err(|_| failed())?;
        loop {
            if self.stopped.load(Ordering::Acquire) || Instant::now() >= call.deadline {
                return Err(failed());
            }
            if let Some(reply) = request.try_reply().map_err(|_| failed())? {
                if reply.request_id != id || reply.generation != self.generation {
                    return Err(failed());
                }
                return reply.result.map_err(|_| failed());
            }
            async_io::Timer::after(Duration::from_millis(2)).await;
        }
    }
    async fn apply(&self, call: &Call, command: ControlCommand) -> fdo::Result<()> {
        match self.request(call, command).await? {
            ControlValue::Acknowledged => Ok(()),
            _ => Err(failed()),
        }
    }
    async fn status(&self, call: &Call) -> fdo::Result<&'static str> {
        match self.request(call, ControlCommand::PlaybackStatus).await? {
            ControlValue::PlaybackStatus(crate::NativePlaybackStatus::Paused) => Ok("Paused"),
            ControlValue::PlaybackStatus(crate::NativePlaybackStatus::Playing) => Ok("Playing"),
            ControlValue::PlaybackStatus(crate::NativePlaybackStatus::Stopped) => Ok("Stopped"),
            _ => Err(failed()),
        }
    }
    async fn pause_command(
        &self,
        command: ControlCommand,
        emitter: &zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let call = self.call()?;
        self.apply(&call, command).await?;
        let status = self.status(&call).await?;
        let properties = zbus::object_server::SignalEmitter::new(emitter.connection(), PATH)
            .map_err(|_| failed())?;
        emit_until(
            &call,
            fdo::Properties::properties_changed(
                &properties,
                "org.mpris.MediaPlayer2.Player"
                    .try_into()
                    .map_err(|_| failed())?,
                HashMap::from([("PlaybackStatus", status.into())]),
                std::borrow::Cow::Borrowed(&[]),
            ),
        )
        .await
    }
    async fn observed_seek(
        &self,
        command: ControlCommand,
        emitter: &zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let call = self.call()?;
        match self.request(&call, command).await? {
            ControlValue::SeekObserved(Some(seconds)) => {
                let position = seconds_to_micros(seconds).map_err(|_| failed())?;
                emit_until(&call, Self::seeked(emitter, position)).await
            }
            ControlValue::SeekObserved(None) => Ok(()),
            _ => Err(failed()),
        }
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.pause_command(ControlCommand::Pause(false), &emitter)
            .await
    }
    async fn pause(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.pause_command(ControlCommand::Pause(true), &emitter)
            .await
    }
    async fn play_pause(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.pause_command(ControlCommand::TogglePause, &emitter)
            .await
    }
    async fn seek(
        &self,
        offset: i64,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.observed_seek(
            ControlCommand::SeekRelativeSeconds(offset as f64 / 1e6),
            &emitter,
        )
        .await
    }
    async fn set_position(
        &self,
        track_id: ObjectPath<'_>,
        position: i64,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        // MPRIS explicitly requires stale IDs and negative positions to be ignored.
        if track_id.as_str() != MPRIS_TRACK_PATH || position < 0 {
            return Ok(());
        }
        let seconds = micros_to_seconds(position).map_err(|_| failed())?;
        self.observed_seek(ControlCommand::SeekObservedSeconds(seconds), &emitter)
            .await
    }
    #[zbus(signal)]
    async fn seeked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        position: i64,
    ) -> zbus::Result<()>;
    fn next(&self) -> fdo::Result<()> {
        Err(unsupported())
    }
    fn previous(&self) -> fdo::Result<()> {
        Err(unsupported())
    }
    fn stop(&self) -> fdo::Result<()> {
        Err(unsupported())
    }
    fn open_uri(&self, _uri: &str) -> fdo::Result<()> {
        Err(unsupported())
    }
    #[zbus(property)]
    async fn playback_status(&self) -> fdo::Result<&'static str> {
        let call = self.call()?;
        self.status(&call).await
    }
    #[zbus(property(emits_changed_signal = "false"))]
    async fn position(&self) -> fdo::Result<i64> {
        let call = self.call()?;
        match self.request(&call, ControlCommand::Position).await? {
            ControlValue::Position(Some(seconds)) => {
                seconds_to_micros(seconds).map_err(|_| failed())
            }
            _ => Err(failed()),
        }
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<&str, OwnedValue> {
        HashMap::from([(
            "mpris:trackid",
            ObjectPath::try_from(MPRIS_TRACK_PATH)
                .expect("constant object path")
                .into(),
        )])
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    async fn set_rate(
        &self,
        value: f64,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if value == 1.0 {
            Ok(())
        } else if value == 0.0 {
            self.pause_command(ControlCommand::Pause(true), &emitter)
                .await
        } else {
            Err(unsupported())
        }
    }
    #[zbus(property(emits_changed_signal = "invalidates"))]
    async fn volume(&self) -> fdo::Result<f64> {
        let call = self.call()?;
        match self.request(&call, ControlCommand::VolumePercent).await? {
            ControlValue::VolumePercent(percent) => Ok(percent / 100.0),
            _ => Err(failed()),
        }
    }
    #[zbus(property)]
    async fn set_volume(
        &self,
        value: f64,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if !value.is_finite() {
            return Err(fdo::Error::InvalidArgs("Nonfinite volume".into()));
        }
        let call = self.call()?;
        self.apply(
            &call,
            ControlCommand::SetVolumePercent(value.clamp(0.0, 1.0) * 100.0),
        )
        .await?;
        let observed = match self.request(&call, ControlCommand::VolumePercent).await? {
            ControlValue::VolumePercent(percent) => percent / 100.0,
            _ => return Err(failed()),
        };
        let properties = zbus::object_server::SignalEmitter::new(emitter.connection(), PATH)
            .map_err(|_| failed())?;
        emit_until(
            &call,
            fdo::Properties::properties_changed(
                &properties,
                "org.mpris.MediaPlayer2.Player"
                    .try_into()
                    .map_err(|_| failed())?,
                HashMap::from([("Volume", observed.into())]),
                std::borrow::Cow::Borrowed(&[]),
            ),
        )
        .await
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) -> fdo::Result<()> {
        Err(unsupported())
    }
    fn quit(&self) -> fdo::Result<()> {
        Err(unsupported())
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Feishin P01 private-bus probe"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec![]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![]
    }
}

/// Blocking construction/teardown for an explicit developer-owned Unix bus only.
/// Run outside UI/async executors. Drop rejects in-flight calls and closes service.
/// The two-second budget covers routed calls, not bus authentication, connection
/// construction/close, OS scheduling or kernel teardown latency.
pub struct MprisProbe {
    connection: Option<zbus::blocking::Connection>,
    stopped: Arc<AtomicBool>,
}
impl MprisProbe {
    pub fn on_owned_bus(
        address: &str,
        name: &str,
        client: ControlClient,
        generation: u64,
    ) -> Result<Self, crate::Error> {
        // Reject multiple/fallback addresses and any non-Unix transport. Ownership
        // is the explicit caller contract; no process environment bus is consulted.
        if !address.starts_with("unix:path=/")
            || address.contains(';')
            || address.len() > 4096
            || address.contains('\0')
            || !name.starts_with("org.mpris.MediaPlayer2.FeishinP01.")
            || name.len() > 255
        {
            return Err(crate::Error::InvalidInput);
        }
        let stopped = Arc::new(AtomicBool::new(false));
        let player = Player {
            client,
            generation,
            next: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(AtomicUsize::new(0)),
            stopped: stopped.clone(),
        };
        let connection = zbus::blocking::connection::Builder::address(address)
            .map_err(|_| crate::Error::InvalidInput)?
            .name(name)
            .map_err(|_| crate::Error::InvalidInput)?
            .serve_at(PATH, Root)
            .map_err(|_| crate::Error::Protocol)?
            .serve_at(PATH, player)
            .map_err(|_| crate::Error::Protocol)?
            .build()
            .map_err(|_| crate::Error::Protocol)?;
        Ok(Self {
            connection: Some(connection),
            stopped,
        })
    }
}
impl Drop for MprisProbe {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(connection) = self.connection.take() {
            let _ = connection.close();
        }
    }
}
