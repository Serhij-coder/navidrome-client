#![cfg(target_os = "linux")]
//! Controlled null-output candidate tests. These do not select a production engine.
use feishin_native_spike::*;
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const BUDGET: Duration = Duration::from_secs(3);
const HELPER: &str = env!("CARGO_BIN_EXE_p01-gst-peer");

fn with_engine(binary: &Path, check: impl FnOnce(&mut Mpv)) {
    with_started(Mpv::start(binary, 17, AudioOutput::Null).unwrap(), check);
}
fn with_loopback_engine(binary: &Path, check: impl FnOnce(&mut Mpv)) {
    with_started(Mpv::start_loopback_probe(binary, 17).unwrap(), check);
}
fn with_started(mut engine: Mpv, check: impl FnOnce(&mut Mpv)) {
    let directory = engine.socket_directory().to_owned();
    let pid = engine.process_id();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&mut engine)));
    let cleanup = engine.shutdown();
    drop(engine);
    assert!(cleanup.is_ok(), "candidate cleanup: {cleanup:?}");
    assert!(!directory.exists(), "candidate socket directory remains");
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "candidate child remains"
    );
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

fn position_until(
    engine: &mut Mpv,
    deadline: Instant,
    accepted: impl Fn(f64) -> bool,
) -> Result<f64, Error> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(Error::Deadline)?;
        if let Some(position) = engine.probe_position(remaining)?
            && accepted(position)
        {
            return Ok(position);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(Duration::from_millis(5).min(remaining));
    }
}

fn stopped_until(engine: &mut Mpv, deadline: Instant) {
    loop {
        match engine.playback_status_until(deadline) {
            Ok(NativePlaybackStatus::Stopped) => return,
            Ok(_) | Err(Error::PropertyUnavailable) => {}
            Err(error) => panic!("EOF observation failed: {error:?}"),
        }
        assert!(Instant::now() < deadline, "EOF observation deadline");
        thread::sleep(
            Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn seek_and_progress(engine: &mut Mpv) -> serde_json::Value {
    let start = Instant::now();
    position_until(engine, start + BUDGET, |position| position.abs() <= 0.1).unwrap();
    let ready_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let deadline = start + BUDGET;
    engine.seek_until(1.25, deadline).unwrap();
    let seek_position =
        position_until(engine, deadline, |position| (position - 1.25).abs() <= 0.1).unwrap();
    let seek_ms = start.elapsed().as_millis();
    engine.pause(false).unwrap();
    position_until(engine, Instant::now() + BUDGET, |position| position >= 1.5).unwrap();
    engine.pause(true).unwrap();
    assert!(engine.paused_until(Instant::now() + BUDGET).unwrap());
    let paused = engine.position().unwrap();
    thread::sleep(Duration::from_millis(40));
    assert!((engine.position().unwrap() - paused).abs() < 0.05);
    json!({"ready_ms":ready_ms,"seek_ms":seek_ms,"seek_position_seconds":seek_position,
        "paused_position_seconds":paused})
}

#[test]
fn gst_owned_wav_controls_seek_switch_eof_and_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.wav");
    let second = directory.path().join("second.wav");
    write_tone(&first, 5, 750).unwrap();
    write_tone(&second, 2, 500).unwrap();
    with_engine(Path::new(HELPER), |engine| {
        assert!(engine.accepts_generation(17));
        assert!(!engine.accepts_generation(16));
        engine.pause(true).unwrap();
        engine.load_local(&first, false).unwrap();
        seek_and_progress(engine);
        engine
            .set_volume_percent_until(25.0, Instant::now() + BUDGET)
            .unwrap();
        assert!(
            (engine
                .volume_percent_until(Instant::now() + BUDGET)
                .unwrap()
                - 25.0)
                .abs()
                < 0.001
        );
        assert!(matches!(
            engine.set_volume_percent(-1.0),
            Err(Error::InvalidInput)
        ));
        assert!(engine.accepts_generation(17));
        engine.load_local(&second, false).unwrap();
        position_until(engine, Instant::now() + BUDGET, |position| {
            position.abs() <= 0.1
        })
        .unwrap();
        assert!(engine.paused_until(Instant::now() + BUDGET).unwrap());
        assert_eq!(engine.playlist_count().unwrap(), 1);
        engine.seek(1.75).unwrap();
        position_until(engine, Instant::now() + BUDGET, |position| {
            (position - 1.75).abs() <= 0.1
        })
        .unwrap();
        engine.pause(false).unwrap();
        stopped_until(engine, Instant::now() + BUDGET);
    });
}

#[test]
fn gst_explicit_alternative_after_missing_primary_and_rejected_operations() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("owned.wav");
    write_tone(&source, 5, 750).unwrap();
    assert!(
        Mpv::start(
            &directory.path().join("missing-primary"),
            16,
            AudioOutput::Null
        )
        .is_err()
    );
    // This is explicit caller selection, not automatic fallback/source restoration.
    with_engine(Path::new(HELPER), |engine| {
        engine.pause(true).unwrap();
        engine.load_local(&source, false).unwrap();
        seek_and_progress(engine);
    });
    for append in [false, true] {
        with_engine(Path::new(HELPER), |engine| {
            engine.pause(true).unwrap();
            engine.load_local(&source, false).unwrap();
            let result = if append {
                engine.load_local(&source, true)
            } else {
                engine.low_pass_probe()
            };
            assert!(
                result.is_err(),
                "unsupported operation must not report success"
            );
            assert!(!engine.accepts_generation(17));
        });
    }
    with_engine(Path::new(HELPER), |engine| {
        assert!(
            engine
                .load_local(&directory.path().join("missing-source"), false)
                .is_err()
        );
        assert!(
            engine.accepts_generation(17),
            "local validation must not retire an untouched child"
        );
        engine.terminate_for_probe().unwrap();
        assert!(!engine.accepts_generation(17));
    });
}

struct HttpFixture {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    served: Arc<AtomicUsize>,
    rejected: Arc<AtomicUsize>,
    ranges: Arc<AtomicUsize>,
    redirected_target: Arc<AtomicUsize>,
    unexpected_errors: Arc<AtomicUsize>,
    _identity: tempfile::NamedTempFile,
}
impl HttpFixture {
    fn start(files: BTreeMap<String, Vec<u8>>) -> Self {
        assert!(files.values().all(|bytes| bytes.len() <= 2 * 1024 * 1024));
        let identity = tempfile::NamedTempFile::new().unwrap();
        let token = identity
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let served = Arc::new(AtomicUsize::new(0));
        let rejected = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(AtomicUsize::new(0));
        let redirected_target = Arc::new(AtomicUsize::new(0));
        let unexpected_errors = Arc::new(AtomicUsize::new(0));
        let flags = (
            stop.clone(),
            served.clone(),
            rejected.clone(),
            ranges.clone(),
            redirected_target.clone(),
            unexpected_errors.clone(),
        );
        let expected = token.clone();
        let handle = thread::spawn(move || {
            while !flags.0.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if let Err(error) = serve(
                            &mut stream,
                            &files,
                            &expected,
                            port,
                            &flags.1,
                            &flags.2,
                            &flags.3,
                            &flags.4,
                        ) {
                            // Seeking/teardown may close a request while this owned
                            // server is reading/writing it. Other I/O failures are
                            // recorded and asserted rather than silently ignored.
                            if !matches!(
                                error.kind(),
                                std::io::ErrorKind::BrokenPipe
                                    | std::io::ErrorKind::ConnectionReset
                                    | std::io::ErrorKind::UnexpectedEof
                                    | std::io::ErrorKind::TimedOut
                                    | std::io::ErrorKind::WouldBlock
                            ) {
                                flags.5.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => {
                        flags.5.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });
        Self {
            port,
            token,
            stop,
            handle: Some(handle),
            served,
            rejected,
            ranges,
            redirected_target,
            unexpected_errors,
            _identity: identity,
        }
    }
    fn assert_healthy(&self) {
        assert_eq!(
            self.unexpected_errors.load(Ordering::Relaxed),
            0,
            "unexpected fixture I/O error"
        );
    }
    fn url(&self, route: &str) -> String {
        format!(
            "http://127.0.0.1:{}/{route}?token={}",
            self.port, self.token
        )
    }
    fn request(&self, method: &str, route: &str, authorized: bool, range: Option<&str>) -> Vec<u8> {
        let mut stream =
            TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let token = if authorized {
            &self.token
        } else {
            "wrong-token"
        };
        let mut request =
            format!("{method} /{route}?token={token} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        if let Some(range) = range {
            request.push_str(&format!("Range: {range}\r\n"));
        }
        request.push_str("Connection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        stream
            .take(2 * 1024 * 1024 + 8192)
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn serve(
    stream: &mut TcpStream,
    files: &BTreeMap<String, Vec<u8>>,
    token: &str,
    port: u16,
    served: &AtomicUsize,
    rejected: &AtomicUsize,
    ranges: &AtomicUsize,
    redirected_target: &AtomicUsize,
) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut request = Vec::new();
    while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Ok(());
        };
        stream.set_read_timeout(Some(remaining))?;
        let mut byte = [0];
        if stream.read(&mut byte)? == 0 {
            return Ok(());
        }
        request.push(byte[0]);
    }
    let Ok(text) = std::str::from_utf8(&request) else {
        return Ok(());
    };
    if !request.ends_with(b"\r\n\r\n") {
        return Ok(());
    }
    let mut lines = text.split("\r\n");
    let Some(first) = lines.next() else {
        return Ok(());
    };
    let parts: Vec<_> = first.split(' ').collect();
    if parts.len() != 3 {
        return Ok(());
    }
    if !matches!(parts[0], "GET" | "HEAD") {
        return response(stream, "405 Method Not Allowed", &[], &[], false);
    }
    let Some((route, query)) = parts[1].split_once('?') else {
        return response(stream, "403 Forbidden", &[], &[], false);
    };
    if query != format!("token={token}") {
        rejected.fetch_add(1, Ordering::Relaxed);
        return response(stream, "403 Forbidden", &[], &[], false);
    }
    if route == "/redirect" {
        return response(
            stream,
            "302 Found",
            &[(
                "Location",
                format!("http://127.0.0.1:{port}/redirect-target?token={token}"),
            )],
            &[],
            false,
        );
    }
    if route == "/redirect-target" {
        redirected_target.fetch_add(1, Ordering::Relaxed);
    }
    let Some(bytes) = files.get(route.trim_start_matches('/')) else {
        return response(stream, "404 Not Found", &[], &[], false);
    };
    served.fetch_add(1, Ordering::Relaxed);
    let range = lines.find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("range"))
            .map(|(_, value)| value.trim())
    });
    let mut headers = vec![("Accept-Ranges", "bytes".into())];
    let body = if let Some(range) = range {
        let Some((start, end)) = range
            .strip_prefix("bytes=")
            .and_then(|value| value.split_once('-'))
        else {
            return response(stream, "416 Range Not Satisfiable", &[], &[], false);
        };
        let Some(start) = start.parse::<usize>().ok() else {
            return response(stream, "416 Range Not Satisfiable", &[], &[], false);
        };
        let end = if end.is_empty() {
            bytes.len().saturating_sub(1)
        } else {
            match end.parse::<usize>() {
                Ok(end) => end,
                Err(_) => return response(stream, "416 Range Not Satisfiable", &[], &[], false),
            }
        };
        if start > end || end >= bytes.len() {
            return response(stream, "416 Range Not Satisfiable", &[], &[], false);
        }
        ranges.fetch_add(1, Ordering::Relaxed);
        headers.push((
            "Content-Range",
            format!("bytes {start}-{end}/{}", bytes.len()),
        ));
        &bytes[start..=end]
    } else {
        bytes.as_slice()
    };
    response(
        stream,
        if range.is_some() {
            "206 Partial Content"
        } else {
            "200 OK"
        },
        &headers,
        body,
        parts[0] == "HEAD",
    )
}
fn response(
    stream: &mut TcpStream,
    status: &str,
    headers: &[(&str, String)],
    body: &[u8],
    head: bool,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    )?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    stream.write_all(b"\r\n")?;
    if !head {
        stream.write_all(body)?;
    }
    Ok(())
}

#[test]
fn owned_http_fixture_enforces_authorization_ranges_and_bounds() {
    let fixture = HttpFixture::start(BTreeMap::from([(
        "owned.wav".into(),
        b"0123456789".to_vec(),
    )]));
    let partial = fixture.request("GET", "owned.wav", true, Some("bytes=2-4"));
    assert!(partial.starts_with(b"HTTP/1.1 206 Partial Content\r\n"));
    assert!(partial.ends_with(b"\r\n\r\n234"));
    assert!(
        fixture
            .request("GET", "owned.wav", false, None)
            .starts_with(b"HTTP/1.1 403")
    );
    assert!(
        fixture
            .request("GET", "owned.wav", true, Some("bytes=99-"))
            .starts_with(b"HTTP/1.1 416")
    );
    assert!(
        fixture
            .request("POST", "owned.wav", true, None)
            .starts_with(b"HTTP/1.1 405")
    );
    assert!(
        fixture
            .request("GET", "missing", true, None)
            .starts_with(b"HTTP/1.1 404")
    );
    let head = fixture.request("HEAD", "owned.wav", true, None);
    assert!(head.starts_with(b"HTTP/1.1 200"));
    assert!(head.ends_with(b"\r\n\r\n"));
    assert_eq!(fixture.ranges.load(Ordering::Relaxed), 1);
    assert_eq!(fixture.rejected.load(Ordering::Relaxed), 1);
    fixture.assert_healthy();
}

#[test]
fn gst_loopback_authentication_and_bad_media_failures_are_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("owned.wav");
    write_tone(&source, 5, 750).unwrap();
    let bytes = std::fs::read(&source).unwrap();
    let fixture = HttpFixture::start(BTreeMap::from([
        ("owned.wav".into(), bytes.clone()),
        ("redirect-target".into(), bytes),
        ("malformed.wav".into(), b"not-a-media-stream".to_vec()),
    ]));
    with_loopback_engine(Path::new(HELPER), |engine| {
        engine.pause(true).unwrap();
        engine
            .load_loopback_probe_url(&fixture.url("owned.wav"), false)
            .unwrap();
        seek_and_progress(engine);
    });
    for url in [
        fixture
            .url("owned.wav")
            .replace(&fixture.token, "wrong-token"),
        fixture.url("malformed.wav"),
        fixture.url("redirect"),
    ] {
        with_loopback_engine(Path::new(HELPER), |engine| {
            engine.pause(true).unwrap();
            let result = engine.load_loopback_probe_url(&url, false);
            assert!(
                result.is_err()
                    || position_until(engine, Instant::now() + BUDGET, |_| true).is_err(),
                "bad source must not become ready"
            );
        });
    }
    assert!(fixture.served.load(Ordering::Relaxed) > 0);
    assert!(fixture.rejected.load(Ordering::Relaxed) > 0);
    assert_eq!(
        fixture.redirected_target.load(Ordering::Relaxed),
        0,
        "candidate followed a redirect"
    );
    fixture.assert_healthy();
}

fn encode(source: &Path, output: &Path) {
    let mut child = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(source)
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("external FFmpeg required");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "fixture encoding failed");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("fixture encoding deadline");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "requires external mpv and FFmpeg; generated codecs/authenticated loopback/null output only"]
fn real_gst_and_mpv_authenticated_codec_comparison() {
    let directory = tempfile::tempdir().unwrap();
    let wav = directory.path().join("owned.wav");
    write_tone(&wav, 5, 750).unwrap();
    let flac = directory.path().join("owned.flac");
    let mp3 = directory.path().join("owned.mp3");
    encode(&wav, &flac);
    encode(&wav, &mp3);
    let sources = [("wav", wav), ("flac", flac), ("mp3", mp3)];
    let files = sources
        .iter()
        .map(|(codec, path)| (format!("owned.{codec}"), std::fs::read(path).unwrap()))
        .collect();
    let fixture = HttpFixture::start(files);
    let mpv = PathBuf::from(std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into()));
    let mut cases = Vec::new();
    for (name, binary) in [("gstreamer", Path::new(HELPER)), ("mpv", mpv.as_path())] {
        for (codec, path) in &sources {
            for network in [false, true] {
                let run = |engine: &mut Mpv| {
                    engine.pause(true).unwrap();
                    let load_start = Instant::now();
                    if network {
                        engine
                            .load_loopback_probe_url(&fixture.url(&format!("owned.{codec}")), false)
                            .unwrap();
                    } else {
                        engine.load_local(path, false).unwrap();
                    }
                    let load_ack_ms = load_start.elapsed().as_millis();
                    let observations = seek_and_progress(engine);
                    cases.push(json!({"engine":name,"codec":codec,"source":if network {"authenticated-loopback"} else {"local"},"load_ack_ms":load_ack_ms,"observations":observations}));
                };
                if network {
                    with_loopback_engine(binary, run);
                } else {
                    with_engine(binary, run);
                }
            }
        }
    }
    assert_eq!(cases.len(), 12);
    assert!(fixture.served.load(Ordering::Relaxed) >= 6);
    fixture.assert_healthy();
    if let Some(path) = std::env::var_os("P01_GST_OUTPUT") {
        let record = json!({"schema":1,"scope":"Owned WAV/FLAC/MP3, two engines, local/authenticated loopback, null output; no production fallback or device/audio certification", "cases":cases,"http_range_requests":fixture.ranges.load(Ordering::Relaxed)});
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        serde_json::to_writer_pretty(&mut output, &record).unwrap();
        output.write_all(b"\n").unwrap();
    }
}

#[test]
fn gst_helper_rejects_system_output_and_bounded_malformed_ipc() {
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;
    struct Peer(std::process::Child);
    impl Drop for Peer {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    fn wait_exit(peer: &mut Peer) -> std::process::ExitStatus {
        let deadline = Instant::now() + BUDGET;
        loop {
            if let Some(status) = peer.0.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "malformed IPC child exit deadline"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let endpoint = directory.path().join("peer.sock");
    let mut rejected = Peer(
        Command::new(HELPER)
            .arg(format!("--input-ipc-server={}", endpoint.display()))
            .arg("--ao=pulse")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    assert!(!wait_exit(&mut rejected).success());
    assert!(!endpoint.exists());
    for payload in [
        b"not-json\n".to_vec(),
        b"{\"command\":[\"get_property\",\"pause\"],\"request_id\":\"invalid\"}\n".to_vec(),
        vec![b'x'; 64 * 1024 + 1],
    ] {
        let directory = tempfile::tempdir().unwrap();
        let endpoint = directory.path().join("peer.sock");
        let mut peer = Peer(
            Command::new(HELPER)
                .arg(format!("--input-ipc-server={}", endpoint.display()))
                .arg("--ao=null")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + BUDGET;
        let mut socket = loop {
            match UnixStream::connect(&endpoint) {
                Ok(socket) => break socket,
                Err(_) => {
                    assert!(
                        peer.0.try_wait().unwrap().is_none(),
                        "helper exited before IPC ready"
                    );
                    assert!(Instant::now() < deadline, "helper IPC startup deadline");
                    thread::sleep(Duration::from_millis(2));
                }
            }
        };
        socket.set_write_timeout(Some(BUDGET)).unwrap();
        if let Err(error) = socket.write_all(&payload) {
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ));
        }
        let _ = socket.shutdown(Shutdown::Write);
        assert!(!wait_exit(&mut peer).success());
    }
}

#[test]
fn gst_control_session_reuses_typed_owner_and_rejects_retired_work() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("owned.wav");
    write_tone(&source, 5, 750).unwrap();
    let session = ControlSession::start(PathBuf::from(HELPER), 17, AudioOutput::Null).unwrap();
    let client = session.client();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let issue = |id, command| {
            let deadline = Instant::now() + BUDGET;
            let reply = client
                .submit(17, id, BUDGET, command)
                .unwrap()
                .wait(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            assert_eq!((reply.generation, reply.request_id), (17, id));
            reply.result.unwrap()
        };
        assert!(matches!(
            issue(1, ControlCommand::Pause(true)),
            ControlValue::Acknowledged
        ));
        assert!(matches!(
            issue(2, ControlCommand::LoadLocal(source)),
            ControlValue::Acknowledged
        ));
        let stale = client
            .submit(16, 99, BUDGET, ControlCommand::Pause(false))
            .unwrap()
            .wait(BUDGET)
            .unwrap();
        assert_eq!((stale.generation, stale.request_id), (16, 99));
        assert!(matches!(stale.result, Err(ControlFailure::Stale)));
        assert!(matches!(
            issue(3, ControlCommand::SeekObservedSeconds(1.25)),
            ControlValue::SeekObserved(Some(position)) if (position - 1.25).abs() <= 0.1
        ));
        assert!(matches!(
            issue(4, ControlCommand::Paused),
            ControlValue::Paused(true)
        ));
        assert!(matches!(
            issue(5, ControlCommand::PlaybackStatus),
            ControlValue::PlaybackStatus(NativePlaybackStatus::Paused)
        ));
    }));
    session.shutdown();
    let cleanup = session.wait_shutdown(Duration::from_secs(8));
    assert!(
        cleanup.is_ok(),
        "typed candidate owner cleanup: {cleanup:?}"
    );
    assert!(matches!(
        client.submit(17, 6, BUDGET, ControlCommand::Position),
        Err(ControlFailure::Stopped)
    ));
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
