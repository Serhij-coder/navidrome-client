//! Isolated null-output GStreamer candidate, speaking only the P01 IPC subset.
//! The managed parent owns deadlines and process kill/reap. Native API calls are
//! confined here because their timing cannot be bounded by Rust thread teardown.
#[cfg(target_os = "linux")]
mod linux {
    use gstreamer::{self as gst, prelude::*};
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    const MAX_LINE: usize = 64 * 1024;
    const STATE_WAIT: gst::ClockTime = gst::ClockTime::from_seconds(2);
    type ProbeResult<T> = Result<T, &'static str>;

    struct Player {
        playbin: gst::Element,
        failed: Arc<AtomicBool>,
        ended: Arc<AtomicBool>,
        paused: bool,
        loaded: bool,
    }

    impl Player {
        fn new() -> ProbeResult<Self> {
            gst::init().map_err(|_| "failure")?;
            let playbin = gst::ElementFactory::make("playbin")
                .build()
                .map_err(|_| "failure")?;
            for property in ["audio-sink", "video-sink"] {
                let sink = gst::ElementFactory::make("fakesink")
                    .property("sync", true)
                    .build()
                    .map_err(|_| "failure")?;
                playbin.set_property(property, &sink);
            }
            let failed = Arc::new(AtomicBool::new(false));
            let ended = Arc::new(AtomicBool::new(false));
            let error_flag = failed.clone();
            let eos_flag = ended.clone();
            let bus = playbin.bus().ok_or("failure")?;
            // No unbounded native message queue or second playback state owner.
            // Only terminal facts cross from streaming threads to this owner.
            bus.set_sync_handler(move |_, message| {
                match message.view() {
                    gst::MessageView::Error(_) => error_flag.store(true, Ordering::Release),
                    gst::MessageView::Eos(_) => eos_flag.store(true, Ordering::Release),
                    _ => {}
                }
                gst::BusSyncReply::Drop
            });
            let source_error = failed.clone();
            playbin.connect("source-setup", false, move |values| {
                let Ok(source) = values[1].get::<gst::Element>() else {
                    source_error.store(true, Ordering::Release);
                    return None;
                };
                let factory = source.factory().map(|factory| factory.name());
                if factory.as_deref() == Some("souphttpsrc") {
                    // The URI boundary admits only explicit loopback fixtures.
                    // Do not permit a fixture response to redirect elsewhere.
                    for name in ["automatic-redirect", "timeout", "proxy"] {
                        if source.find_property(name).is_none() {
                            source_error.store(true, Ordering::Release);
                            return None;
                        }
                    }
                    source.set_property("automatic-redirect", false);
                    source.set_property("timeout", 2_u32);
                    source.set_property("proxy", "");
                } else if factory.as_deref() != Some("filesrc") {
                    source_error.store(true, Ordering::Release);
                }
                None
            });
            Ok(Self {
                playbin,
                failed,
                ended,
                paused: false,
                loaded: false,
            })
        }

        fn state(&self, wanted: gst::State) -> ProbeResult<()> {
            self.playbin.set_state(wanted).map_err(|_| "failure")?;
            let (result, current, _) = self.playbin.state(STATE_WAIT);
            result.map_err(|_| "failure")?;
            if current != wanted || self.failed.load(Ordering::Acquire) {
                return Err("failure");
            }
            Ok(())
        }

        fn load(&mut self, input: &str) -> ProbeResult<Value> {
            if input.is_empty() || input.len() > 4096 || input.contains('\0') {
                return Err("failure");
            }
            let uri = if input.starts_with("http:") {
                feishin_native_spike::validate_loopback_probe_url(input).map_err(|_| "failure")?;
                input.to_owned()
            } else {
                let path = Path::new(input).canonicalize().map_err(|_| "failure")?;
                if !path.is_file() {
                    return Err("failure");
                }
                gst::glib::filename_to_uri(path, None)
                    .map_err(|_| "failure")?
                    .into()
            };
            self.state(gst::State::Null)?;
            self.loaded = false;
            self.failed.store(false, Ordering::Release);
            self.ended.store(false, Ordering::Release);
            self.playbin.set_property("uri", uri);
            self.state(gst::State::Paused)?;
            self.loaded = true;
            if !self.paused {
                self.state(gst::State::Playing)?;
            }
            Ok(Value::Null)
        }

        fn property(&self, property: &str) -> ProbeResult<Value> {
            if property == "mpv-version" {
                return Ok(json!(format!("P01 GStreamer {}", gst::version_string())));
            }
            if self.failed.load(Ordering::Acquire) {
                return Err("failure");
            }
            match property {
                "pause" => Ok(json!(self.paused)),
                "idle-active" => Ok(json!(!self.loaded || self.ended.load(Ordering::Acquire))),
                "volume" => Ok(json!(self.playbin.property::<f64>("volume") * 100.0)),
                "playlist-count" => Ok(json!(u64::from(self.loaded))),
                "duration" if self.loaded => self
                    .playbin
                    .query_duration::<gst::ClockTime>()
                    .map(|value| json!(value.seconds_f64()))
                    .ok_or("property unavailable"),
                "time-pos" if self.loaded => self
                    .playbin
                    .query_position::<gst::ClockTime>()
                    .map(|value| json!(value.seconds_f64()))
                    .ok_or("property unavailable"),
                "duration" | "time-pos" => Err("property unavailable"),
                _ => Err("failure"),
            }
        }

        fn command(&mut self, command: &[Value]) -> ProbeResult<Value> {
            match command.first().and_then(Value::as_str) {
                Some("get_property") if command.len() == 2 => {
                    self.property(command[1].as_str().ok_or("failure")?)
                }
                Some("set_property") if command.len() == 3 => {
                    match command[1].as_str() {
                        Some("pause") => {
                            let paused = command[2].as_bool().ok_or("failure")?;
                            if self.loaded {
                                self.state(if paused {
                                    gst::State::Paused
                                } else {
                                    gst::State::Playing
                                })?;
                            }
                            self.paused = paused;
                        }
                        Some("volume") => {
                            let volume = command[2]
                                .as_f64()
                                .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
                                .ok_or("failure")?;
                            self.playbin.set_property("volume", volume / 100.0);
                        }
                        _ => return Err("failure"),
                    }
                    Ok(Value::Null)
                }
                Some("loadfile") if command.len() == 3 && command[2] == "replace" => {
                    self.load(command[1].as_str().ok_or("failure")?)
                }
                Some("seek") if command.len() == 3 && command[2] == "absolute+exact" => {
                    let seconds = command[1].as_f64().ok_or("failure")?;
                    let position =
                        gst::ClockTime::try_from_seconds_f64(seconds).map_err(|_| "failure")?;
                    if !self.loaded || self.failed.load(Ordering::Acquire) {
                        return Err("failure");
                    }
                    self.playbin
                        .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position)
                        .map_err(|_| "failure")?;
                    self.ended.store(false, Ordering::Release);
                    Ok(Value::Null)
                }
                _ => Err("failure"),
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            // The parent kills/reaps this isolated process if native teardown
            // stalls; never move this blocking boundary onto the UI thread.
            let _ = self.playbin.set_state(gst::State::Null);
        }
    }

    fn endpoint() -> ProbeResult<PathBuf> {
        let mut endpoint = None;
        let mut null_output = false;
        for argument in std::env::args().skip(1) {
            if let Some(path) = argument.strip_prefix("--input-ipc-server=") {
                if endpoint.is_some() || path.is_empty() || path.len() > 4096 {
                    return Err("failure");
                }
                endpoint = Some(PathBuf::from(path));
            } else if argument == "--ao=null" {
                null_output = true;
            } else if ![
                "--no-config",
                "--load-scripts=no",
                "--idle=yes",
                "--no-terminal",
                "--really-quiet",
                "--vid=no",
                "--force-window=no",
                "--input-media-keys=yes",
            ]
            .contains(&argument.as_str())
            {
                return Err("failure");
            }
        }
        let endpoint = endpoint.ok_or("failure")?;
        if !null_output || !endpoint.is_absolute() {
            return Err("failure");
        }
        Ok(endpoint)
    }

    // Bound before extending the allocation, including partial lines.
    fn line(reader: &mut impl BufRead) -> std::io::Result<Option<Vec<u8>>> {
        let mut output = Vec::new();
        loop {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return if output.is_empty() {
                    Ok(None)
                } else {
                    Err(std::io::ErrorKind::UnexpectedEof.into())
                };
            }
            let count = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if count > MAX_LINE - output.len() {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            output.extend_from_slice(&available[..count]);
            reader.consume(count);
            if output.ends_with(b"\n") {
                return Ok(Some(output));
            }
        }
    }

    pub fn run() -> ProbeResult<()> {
        let path = endpoint()?;
        let mut player = Player::new()?;
        let listener = UnixListener::bind(path).map_err(|_| "failure")?;
        let (mut socket, _) = listener.accept().map_err(|_| "failure")?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| "failure")?;
        let mut reader = BufReader::new(socket.try_clone().map_err(|_| "failure")?);
        while let Some(bytes) = line(&mut reader).map_err(|_| "failure")? {
            let request: Value = serde_json::from_slice(&bytes).map_err(|_| "failure")?;
            let command = request
                .get("command")
                .and_then(Value::as_array)
                .ok_or("failure")?;
            if command.len() == 1 && command[0] == "quit" {
                break;
            }
            let id = request
                .get("request_id")
                .and_then(Value::as_u64)
                .ok_or("failure")?;
            let (error, data) = match player.command(command) {
                Ok(data) => ("success", data),
                Err(error) => (error, Value::Null),
            };
            let mut response = serde_json::to_vec(&json!({
                "request_id": id, "error": error, "data": data
            }))
            .map_err(|_| "failure")?;
            response.push(b'\n');
            socket.write_all(&response).map_err(|_| "failure")?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if linux::run().is_err() {
        // No paths, authenticated URLs, native diagnostics or request payloads.
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
