//! Native P01 experiments. Blocking APIs must run outside UI/event executors.
use serde_json::{Value, json};
use std::{
    fmt, io,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(any(unix, windows))]
mod mpv;
#[cfg(any(unix, windows))]
pub use mpv::{AudioOutput, Mpv, NativePlaybackStatus};
mod capture;
pub use capture::Capture;
mod analysis_session;
pub use analysis_session::{AnalysisSession, AnalysisSource, AnalysisStatus};
mod ffplay;
pub use ffplay::{FfplayOutput, FfplayProbe};

#[derive(Debug)]
pub enum Error {
    Io(io::ErrorKind),
    Deadline,
    ProcessExited,
    Protocol,
    Rejected,
    PropertyUnavailable,
    Unsupported,
    InvalidInput,
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never interpolate paths, URLs, child stderr or protocol payloads here.
        write!(f, "native probe failure: {self:?}")
    }
}
impl std::error::Error for Error {}

pub(crate) struct ManagedChild(pub(crate) Child);
impl std::ops::Deref for ManagedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for ManagedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = stop_child(&mut self.0, Duration::ZERO);
    }
}

pub(crate) fn stop_child(child: &mut Child, timeout: Duration) -> Result<(), Error> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    match child.kill() {
        Ok(()) => {
            child.wait()?;
            Ok(())
        }
        Err(error) => {
            if child.try_wait()?.is_some() {
                Ok(())
            } else {
                Err(error.into())
            }
        }
    }
}

/// Generate owned PCM tones for disposable playback/capture measurements.
pub fn write_tone(path: &Path, seconds: u16, frequency_hz: u16) -> Result<(), Error> {
    use std::io::Write;
    if seconds == 0 || seconds > 30 || frequency_hz == 0 || frequency_hz > 20_000 {
        return Err(Error::InvalidInput);
    }
    let samples = u32::from(seconds) * 48_000;
    let data_size = samples * 2;
    let mut out = std::fs::File::create_new(path)?;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_size).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16_u32.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&48_000_u32.to_le_bytes())?;
    out.write_all(&96_000_u32.to_le_bytes())?;
    out.write_all(&2_u16.to_le_bytes())?;
    out.write_all(&16_u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data_size.to_le_bytes())?;
    for sample in 0..samples {
        let value =
            ((std::f64::consts::TAU * f64::from(frequency_hz) * f64::from(sample) / 48_000.0).sin()
                * 8192.0) as i16;
        out.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

/// A small structured event vocabulary; no free-form user/network values are logged.
#[derive(Clone, Copy, Debug)]
pub enum LogEvent {
    Started,
    Ready,
    Stopped,
    Failed,
}
pub fn log_event(event: LogEvent, generation: u64) -> Value {
    json!({"component": "p01-probe", "event": match event { LogEvent::Started => "started", LogEvent::Ready => "ready", LogEvent::Stopped => "stopped", LogEvent::Failed => "failed" }, "generation": generation})
}

/// Child executable is an explicit trusted developer tool, never a shell command.
pub(crate) fn child_command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[cfg(any(unix, windows))]
mod control_session;
#[cfg(any(unix, windows))]
pub use control_session::{
    CONTROL_CAPACITY, ControlClient, ControlCommand, ControlFailure, ControlReply, ControlRequest,
    ControlSession, ControlValue,
};

#[cfg(all(target_os = "linux", feature = "mpris-probe"))]
mod mpris;
#[cfg(all(target_os = "linux", feature = "mpris-probe"))]
pub use mpris::{MPRIS_TRACK_PATH, MprisProbe, micros_to_seconds, seconds_to_micros};

#[cfg(any(unix, windows))]
mod handoff;
#[cfg(any(unix, windows))]
pub use handoff::{HANDOFF_CAPACITY, HandoffFailure, HandoffReply, HandoffRequest, HandoffSession};
