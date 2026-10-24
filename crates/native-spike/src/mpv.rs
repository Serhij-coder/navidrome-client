use super::*;
use std::io::{BufRead, BufReader, Write};

mod transport;
use transport::{Connector, Stream};

const MAX_LINE: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
pub enum AudioOutput {
    System,
    Null,
    Pulse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

pub struct Mpv {
    child: ManagedChild,
    socket: BufReader<Stream>,
    directory: tempfile::TempDir,
    request: u64,
    generation: u64,
    active: bool,
}

impl Mpv {
    pub fn start(binary: &Path, generation: u64, output: AudioOutput) -> Result<Self, Error> {
        let directory = ipc_directory()?;
        let path = transport::endpoint(&directory)?;
        let connector = Connector::new()?;
        let mut command = child_command(binary);
        command.args([
            "--no-config",
            "--load-scripts=no",
            "--idle=yes",
            "--no-terminal",
            "--really-quiet",
            "--vid=no",
            "--force-window=no",
            "--input-media-keys=yes",
        ]);
        command.arg(format!("--input-ipc-server={}", path.display()));
        match output {
            AudioOutput::System => {}
            AudioOutput::Null => {
                command.arg("--ao=null");
            }
            AudioOutput::Pulse => {
                command.arg("--ao=pulse");
            }
        }
        let mut child = ManagedChild(command.spawn()?);
        let deadline = Instant::now() + DEADLINE;
        loop {
            match connector.connect(&path, child.id()) {
                Ok(socket) => {
                    socket.set_write_timeout(Some(DEADLINE))?;
                    let mut mpv = Self {
                        child,
                        socket: BufReader::new(socket),
                        directory,
                        request: 0,
                        generation,
                        active: true,
                    };
                    mpv.command(json!(["get_property", "mpv-version"]))?;
                    return Ok(mpv);
                }
                Err(error) => {
                    if error.kind() == io::ErrorKind::PermissionDenied {
                        return Err(error.into());
                    }
                    if child.try_wait()?.is_some() {
                        return Err(Error::ProcessExited);
                    }
                    if Instant::now() >= deadline {
                        stop_child(&mut child, Duration::ZERO)?;
                        return Err(Error::Deadline);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    fn command(&mut self, command: Value) -> Result<Value, Error> {
        let result = self.exchange(command);
        if result.is_err() {
            self.active = false;
        }
        result
    }
    fn command_until(&mut self, command: Value, deadline: Instant) -> Result<Value, Error> {
        remaining_time(deadline)?;
        let result = self.exchange_until(command, deadline.min(Instant::now() + DEADLINE));
        if result.is_err() {
            self.active = false;
        }
        result
    }
    fn exchange(&mut self, command: Value) -> Result<Value, Error> {
        self.exchange_until(command, Instant::now() + DEADLINE)
    }
    fn exchange_until(&mut self, command: Value, deadline: Instant) -> Result<Value, Error> {
        if !self.active {
            return Err(Error::ProcessExited);
        }
        self.request = self.request.checked_add(1).ok_or(Error::Protocol)?;
        let request = self.request;
        let message = json!({"command": command, "request_id": request});
        let mut bytes = serde_json::to_vec(&message).map_err(|_| Error::Protocol)?;
        bytes.push(b'\n');
        let mut pending = bytes.as_slice();
        while !pending.is_empty() {
            let remaining = remaining_time(deadline)?;
            self.socket.get_mut().set_write_timeout(Some(remaining))?;
            match self.socket.get_mut().write(pending) {
                Ok(0) => return Err(Error::ProcessExited),
                Ok(written) => pending = &pending[written..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(deadline_error(error)),
            }
        }
        loop {
            let mut line = Vec::new();
            loop {
                // Recompute before every underlying read, including partial lines.
                let remaining = remaining_time(deadline)?;
                self.socket.get_mut().set_read_timeout(Some(remaining))?;
                let available = match self.socket.fill_buf() {
                    Ok([]) => return Err(Error::ProcessExited),
                    Ok(available) => available,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(deadline_error(error)),
                };
                let length = available
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(available.len(), |index| index + 1);
                if length > MAX_LINE - line.len() {
                    return Err(Error::Protocol);
                }
                line.extend_from_slice(&available[..length]);
                self.socket.consume(length);
                if line.ends_with(b"\n") {
                    break;
                }
            }
            remaining_time(deadline)?;
            let reply: Value = serde_json::from_slice(&line).map_err(|_| Error::Protocol)?;
            if reply.get("request_id").and_then(Value::as_u64) != Some(request) {
                continue;
            }
            match reply.get("error").and_then(Value::as_str) {
                Some("success") => {}
                Some("property unavailable") => return Err(Error::PropertyUnavailable),
                _ => return Err(Error::Rejected),
            }
            return Ok(reply.get("data").cloned().unwrap_or(Value::Null));
        }
    }

    pub fn load_local(&mut self, path: &Path, append: bool) -> Result<(), Error> {
        self.load_local_until(path, append, Instant::now() + DEADLINE)
    }
    pub fn load_local_until(
        &mut self,
        path: &Path,
        append: bool,
        deadline: Instant,
    ) -> Result<(), Error> {
        remaining_time(deadline)?;
        // JSON IPC requires Unicode regardless of whether the host filesystem can
        // create or canonicalize an arbitrary Unix filename. Validate at entry.
        if path.to_str().is_none() {
            return Err(Error::InvalidInput);
        }
        let path = path.canonicalize()?;
        if !path.is_file() {
            return Err(Error::InvalidInput);
        }
        let path = path.to_str().ok_or(Error::InvalidInput)?;
        self.command_until(
            json!(["loadfile", path, if append { "append" } else { "replace" }]),
            deadline,
        )?;
        Ok(())
    }
    pub fn pause(&mut self, paused: bool) -> Result<(), Error> {
        self.pause_until(paused, Instant::now() + DEADLINE)
    }
    pub fn pause_until(&mut self, paused: bool, deadline: Instant) -> Result<(), Error> {
        self.command_until(json!(["set_property", "pause", paused]), deadline)?;
        Ok(())
    }
    // A normally unavailable property while loading is not a corrupt transport.
    // Invalid successful DTOs and other errors invalidate this owned IPC session.
    fn read_property_until(&mut self, property: &str, deadline: Instant) -> Result<Value, Error> {
        remaining_time(deadline)?;
        match self.exchange_until(json!(["get_property", property]), deadline) {
            Ok(value) => Ok(value),
            Err(Error::PropertyUnavailable) => Err(Error::PropertyUnavailable),
            Err(error) => {
                self.active = false;
                Err(error)
            }
        }
    }
    fn bool_property_until(&mut self, property: &str, deadline: Instant) -> Result<bool, Error> {
        match self.read_property_until(property, deadline)?.as_bool() {
            Some(value) => Ok(value),
            None => {
                self.active = false;
                Err(Error::Protocol)
            }
        }
    }
    fn nonnegative_property_until(
        &mut self,
        property: &str,
        deadline: Instant,
    ) -> Result<f64, Error> {
        match self
            .read_property_until(property, deadline)?
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0)
        {
            Some(value) => Ok(value),
            None => {
                self.active = false;
                Err(Error::Protocol)
            }
        }
    }
    /// Worker-only observed pause state, under the caller's remaining deadline.
    pub fn paused_until(&mut self, deadline: Instant) -> Result<bool, Error> {
        self.bool_property_until("pause", deadline)
    }
    /// Idle/EOF takes precedence over pause. Missing time-pos during loading is
    /// explicitly unavailable, never inferred as Playing; all reads share budget.
    pub fn playback_status_until(
        &mut self,
        deadline: Instant,
    ) -> Result<NativePlaybackStatus, Error> {
        if self.bool_property_until("idle-active", deadline)? {
            return Ok(NativePlaybackStatus::Stopped);
        }
        self.probe_position(remaining_time(deadline)?)?
            .ok_or(Error::PropertyUnavailable)?;
        let paused = self.paused_until(deadline)?;
        if self.bool_property_until("idle-active", deadline)? {
            return Ok(NativePlaybackStatus::Stopped);
        }
        Ok(if paused {
            NativePlaybackStatus::Paused
        } else {
            NativePlaybackStatus::Playing
        })
    }
    /// Waits for command acknowledgement, not arrival at the requested position.
    pub fn seek(&mut self, seconds: f64) -> Result<(), Error> {
        self.seek_until(seconds, Instant::now() + DEADLINE)
    }
    pub fn seek_until(&mut self, seconds: f64, deadline: Instant) -> Result<(), Error> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(Error::InvalidInput);
        }
        self.command_until(json!(["seek", seconds, "absolute+exact"]), deadline)?;
        Ok(())
    }
    /// Observe a current media position, independently of seek acknowledgement.
    /// `None` means exactly mpv's `property unavailable`, normally while idle or
    /// loading. Other errors and invalid successful values invalidate IPC.
    /// Blocking worker API: the caller's budget is capped at three seconds.
    pub fn probe_position(&mut self, timeout: Duration) -> Result<Option<f64>, Error> {
        if timeout.is_zero() {
            return Err(Error::Deadline);
        }
        let result = self.exchange_until(
            json!(["get_property", "time-pos"]),
            Instant::now() + timeout.min(DEADLINE),
        );
        match result {
            Ok(value) => match value
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
            {
                Some(position) => Ok(Some(position)),
                None => {
                    self.active = false;
                    Err(Error::Protocol)
                }
            },
            Err(Error::PropertyUnavailable) => Ok(None),
            Err(error) => {
                self.active = false;
                Err(error)
            }
        }
    }
    /// P01 single-source seek: ignore out-of-range absolute positions, reject
    /// relative next-track transitions, then observe arrival under one deadline.
    pub fn seek_observed_until(
        &mut self,
        seconds: f64,
        relative: bool,
        deadline: Instant,
    ) -> Result<Option<f64>, Error> {
        if !seconds.is_finite() {
            return Err(Error::InvalidInput);
        }
        let duration = self.nonnegative_property_until("duration", deadline)?;
        if duration == 0.0 {
            return Err(Error::PropertyUnavailable);
        }
        let target = if relative {
            let current = self
                .probe_position(remaining_time(deadline)?)?
                .ok_or(Error::PropertyUnavailable)?;
            let target = current + seconds;
            if !target.is_finite() || target > duration {
                return Err(Error::Unsupported);
            }
            target.max(0.0)
        } else {
            if seconds < 0.0 || seconds > duration {
                return Ok(None);
            }
            seconds
        };
        self.seek_until(target, deadline)?;
        loop {
            if let Some(position) = self.probe_position(remaining_time(deadline)?)?
                && (position - target).abs() <= 0.1
            {
                return Ok(Some(position));
            }
            std::thread::sleep(remaining_time(deadline)?.min(Duration::from_millis(10)));
        }
    }
    pub fn volume_percent_until(&mut self, deadline: Instant) -> Result<f64, Error> {
        self.nonnegative_property_until("volume", deadline)
    }
    pub fn set_volume_percent_until(
        &mut self,
        percent: f64,
        deadline: Instant,
    ) -> Result<(), Error> {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(Error::InvalidInput);
        }
        self.command_until(json!(["set_property", "volume", percent]), deadline)?;
        Ok(())
    }
    pub fn position(&mut self) -> Result<f64, Error> {
        self.probe_position(DEADLINE)?
            .ok_or(Error::PropertyUnavailable)
    }
    pub fn playlist_count(&mut self) -> Result<u64, Error> {
        self.command(json!(["get_property", "playlist-count"]))?
            .as_u64()
            .ok_or(Error::Protocol)
    }
    /// mpv's percentage property, not an assumed linear amplitude gain law.
    pub fn set_volume_percent(&mut self, percent: f64) -> Result<(), Error> {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(Error::InvalidInput);
        }
        self.command(json!(["set_property", "volume", percent]))?;
        Ok(())
    }
    pub fn low_pass_probe(&mut self) -> Result<(), Error> {
        self.command(json!(["af", "set", "lavfi=[lowpass=f=2000]"]))?;
        Ok(())
    }
    pub fn accepts_generation(&self, generation: u64) -> bool {
        self.active && generation == self.generation
    }
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
    pub fn socket_directory(&self) -> &Path {
        self.directory.path()
    }
    pub fn terminate_for_probe(&mut self) -> Result<(), Error> {
        self.active = false;
        stop_child(&mut self.child, Duration::ZERO)
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        // Invalidate callbacks before waiting for any IPC or process completion.
        if self.active {
            self.active = false;
            self.socket
                .get_mut()
                .set_write_timeout(Some(Duration::from_millis(100)))?;
            let _ = self
                .socket
                .get_mut()
                .write_all(b"{\"command\":[\"quit\"]}\n");
        }
        stop_child(&mut self.child, Duration::from_millis(500))
    }
}
impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn remaining_time(deadline: Instant) -> Result<Duration, Error> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(Error::Deadline)
}

fn deadline_error(error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Error::Deadline,
        _ => error.into(),
    }
}

#[cfg(unix)]
fn ipc_directory() -> Result<tempfile::TempDir, Error> {
    // macOS/BSD Unix sockets have a smaller pathname limit than Linux. Leave room
    // for the terminating NUL and reject paths that command-line Unicode cannot
    // represent exactly. TempDir creates an unpredictable, mode-0700 directory.
    fn fits(directory: &tempfile::TempDir) -> bool {
        use std::os::unix::ffi::OsStrExt;
        let path = directory.path().join("ipc");
        path.to_str().is_some() && path.as_os_str().as_bytes().len() <= 100
    }
    use std::os::unix::fs::PermissionsExt;
    let mut builder = tempfile::Builder::new();
    builder
        .prefix("feishin-p01-mpv-")
        .permissions(std::fs::Permissions::from_mode(0o700));
    if let Ok(directory) = builder.tempdir()
        && fits(&directory)
    {
        return Ok(directory);
    }
    // Respect a usable configured temp root first. A long/non-Unicode/unusable
    // TMPDIR falls back to a fresh private directory, never a shared socket name.
    let directory = builder.tempdir_in("/tmp")?;
    if !fits(&directory) {
        return Err(Error::InvalidInput);
    }
    Ok(directory)
}

#[cfg(windows)]
fn ipc_directory() -> Result<tempfile::TempDir, Error> {
    Ok(tempfile::Builder::new()
        .prefix("feishin-p01-mpv-")
        .tempdir()?)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../tests/native-spike/transitions.rs"]
mod transition_tests;
