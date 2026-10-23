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
    fn exchange(&mut self, command: Value) -> Result<Value, Error> {
        if !self.active {
            return Err(Error::ProcessExited);
        }
        self.request = self.request.checked_add(1).ok_or(Error::Protocol)?;
        let request = self.request;
        let message = json!({"command": command, "request_id": request});
        let mut bytes = serde_json::to_vec(&message).map_err(|_| Error::Protocol)?;
        bytes.push(b'\n');
        let deadline = Instant::now() + DEADLINE;
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
            if reply.get("error").and_then(Value::as_str) != Some("success") {
                return Err(Error::Rejected);
            }
            return Ok(reply.get("data").cloned().unwrap_or(Value::Null));
        }
    }

    pub fn load_local(&mut self, path: &Path, append: bool) -> Result<(), Error> {
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
        self.command(json!([
            "loadfile",
            path,
            if append { "append" } else { "replace" }
        ]))?;
        Ok(())
    }
    pub fn pause(&mut self, paused: bool) -> Result<(), Error> {
        self.command(json!(["set_property", "pause", paused]))?;
        Ok(())
    }
    pub fn seek(&mut self, seconds: f64) -> Result<(), Error> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(Error::InvalidInput);
        }
        self.command(json!(["seek", seconds, "absolute+exact"]))?;
        Ok(())
    }
    pub fn position(&mut self) -> Result<f64, Error> {
        self.command(json!(["get_property", "time-pos"]))?
            .as_f64()
            .ok_or(Error::Protocol)
    }
    pub fn playlist_count(&mut self) -> Result<u64, Error> {
        self.command(json!(["get_property", "playlist-count"]))?
            .as_u64()
            .ok_or(Error::Protocol)
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
