//! Opt-in live monitor measurements; no physical/device gapless certification.
use super::*;
use std::{
    fs::OpenOptions,
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};

const RATE: usize = 48_000;
const WINDOW: usize = RATE / 1000;
const CAP: usize = RATE * 12;
const QUIET_RMS: f32 = 0.02;

#[derive(Debug, PartialEq)]
struct Energy {
    span_ms: usize,
    active_ms: usize,
    longest_quiet_ms: usize,
}
fn energy(pcm: &[f32]) -> Result<Energy, Error> {
    if pcm.len() > CAP || pcm.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::Protocol);
    }
    let windows: Vec<_> = pcm
        .chunks_exact(WINDOW)
        .map(|chunk| {
            (chunk.iter().map(|sample| sample * sample).sum::<f32>() / WINDOW as f32).sqrt()
                >= QUIET_RMS
        })
        .collect();
    let first = windows
        .iter()
        .position(|active| *active)
        .ok_or(Error::Protocol)?;
    let last = windows
        .iter()
        .rposition(|active| *active)
        .ok_or(Error::Protocol)?;
    let (mut quiet, mut longest, mut active_ms) = (0, 0, 0);
    for active in &windows[first..=last] {
        if *active {
            active_ms += 1;
            quiet = 0;
        } else {
            quiet += 1;
            longest = longest.max(quiet);
        }
    }
    Ok(Energy {
        span_ms: last - first + 1,
        active_ms,
        longest_quiet_ms: longest,
    })
}

struct Monitor {
    child: ManagedChild,
    frames: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<Vec<f32>, Error>>>,
}
impl Monitor {
    fn start() -> Result<Self, Error> {
        Self::start_with_cap(CAP)
    }
    fn start_with_cap(cap: usize) -> Result<Self, Error> {
        if !(RATE..=RATE * 24).contains(&cap) {
            return Err(Error::InvalidInput);
        }
        let mut child = ManagedChild(
            child_command(Path::new("ffmpeg"))
                .args([
                    "-nostdin",
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "pulse",
                    "-i",
                    "p01_transition.monitor",
                    "-ac",
                    "1",
                    "-ar",
                    "48000",
                    "-f",
                    "f32le",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .spawn()?,
        );
        let stdout = child.stdout.take().ok_or(Error::Protocol)?;
        let frames = Arc::new(AtomicUsize::new(0));
        let output = frames.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker =
            std::thread::spawn(move || read_pcm(BufReader::new(stdout), &output, &stop, cap));
        Ok(Self {
            child,
            frames,
            stopped,
            worker: Some(worker),
        })
    }
    fn ready(&mut self) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(4);
        while self.frames.load(Ordering::Acquire) < RATE / 2 {
            if self.child.try_wait()?.is_some() {
                return Err(Error::ProcessExited);
            }
            if Instant::now() >= deadline {
                return Err(Error::Deadline);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Vec<f32>, Error> {
        self.stopped.store(true, Ordering::Release);
        stop_child(&mut self.child, Duration::ZERO)?;
        self.worker
            .take()
            .ok_or(Error::Protocol)?
            .join()
            .map_err(|_| Error::Protocol)?
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = stop_child(&mut self.child, Duration::ZERO);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_pcm(
    mut reader: impl Read,
    frames: &AtomicUsize,
    stopped: &AtomicBool,
    cap: usize,
) -> Result<Vec<f32>, Error> {
    let mut pcm = Vec::with_capacity(cap);
    let mut bytes = [0; 4];
    while pcm.len() < cap {
        match reader.read_exact(&mut bytes) {
            Ok(()) => pcm.push(f32::from_le_bytes(bytes)),
            Err(_) if stopped.load(Ordering::Acquire) => return Ok(pcm),
            Err(_) => return Err(Error::ProcessExited),
        }
        if pcm.len() % 1024 == 0 {
            frames.store(pcm.len(), Ordering::Release);
        }
    }
    Err(Error::Protocol)
}

#[test]
fn premature_capture_eof_is_an_error_even_with_valid_pcm() {
    let frames = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    let bytes = [0.25_f32.to_le_bytes(); 100].concat();
    assert!(matches!(
        read_pcm(std::io::Cursor::new(&bytes), &frames, &stopped, CAP),
        Err(Error::ProcessExited)
    ));
    stopped.store(true, Ordering::Release);
    assert_eq!(
        read_pcm(std::io::Cursor::new(&bytes), &frames, &stopped, CAP)
            .unwrap()
            .len(),
        100
    );
}

fn wait_encoder(child: &mut ManagedChild, timeout: Duration) -> Result<(), Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(Error::Rejected)
            };
        }
        if Instant::now() >= deadline {
            stop_child(child, Duration::ZERO)?;
            return Err(Error::Deadline);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn stalled_fixture_encoder_is_deadlined_and_reaped() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("ffplay-stall");
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_mpv.py"
        ),
        &executable,
    )
    .unwrap();
    let mut child = ManagedChild(child_command(&executable).spawn().unwrap());
    assert!(matches!(
        wait_encoder(&mut child, Duration::from_millis(40)),
        Err(Error::Deadline)
    ));
    assert!(child.try_wait().unwrap().is_some());
}

#[test]
fn energy_windows_measure_known_silence_and_exclude_outer_padding() {
    let mut pcm = vec![0.0; RATE / 10];
    for frequency in [750.0, 1500.0] {
        if frequency == 1500.0 {
            pcm.extend(vec![0.0; WINDOW * 37]);
        }
        pcm.extend((0..RATE).map(|sample| {
            (std::f32::consts::TAU * frequency * sample as f32 / RATE as f32).sin() * 0.25
        }));
    }
    pcm.extend(vec![0.0; RATE / 10]);
    assert_eq!(
        energy(&pcm).unwrap(),
        Energy {
            span_ms: 2037,
            active_ms: 2000,
            longest_quiet_ms: 37
        }
    );
}

#[test]
fn missing_nonfinite_or_over_limit_pcm_is_not_a_zero_gap_result() {
    for pcm in [
        vec![],
        vec![0.0; RATE],
        vec![f32::NAN; WINDOW],
        vec![f32::INFINITY; WINDOW],
        vec![0.25; CAP + 1],
    ] {
        assert!(energy(&pcm).is_err());
    }
}

fn capture_pair(
    binary: &Path,
    paths: &[std::path::PathBuf; 2],
    pause_control: bool,
    prefetch: bool,
) -> Result<Value, Error> {
    let mut monitor = Monitor::start()?;
    monitor.ready()?;
    let mut mpv = Mpv::start(binary, 1, AudioOutput::Pulse)?;
    let version = mpv.command(json!(["get_property", "mpv-version"]))?;
    let gapless = mpv.command(json!(["get_property", "gapless-audio"]))?;
    let default_prefetch = mpv.command(json!(["get_property", "prefetch-playlist"]))?;
    let scripts = mpv.command(json!(["get_property", "load-scripts"]))?;
    assert_eq!(
        scripts.as_bool(),
        Some(false),
        "managed mpv must not autoload profile scripts"
    );
    mpv.command(json!(["set_property", "prefetch-playlist", prefetch]))?;
    let actual_prefetch = mpv.command(json!(["get_property", "prefetch-playlist"]))?;
    assert_eq!(actual_prefetch.as_bool(), Some(prefetch));
    mpv.pause(true)?;
    mpv.load_local(&paths[0], false)?;
    mpv.load_local(&paths[1], true)?;
    assert_eq!(mpv.playlist_count()?, 2);
    mpv.pause(false)?;
    let start = Instant::now();
    let deadline = start + Duration::from_secs(8);
    let (mut seen, mut paused) = ([false; 2], false);
    loop {
        let playlist = mpv.command(json!(["get_property", "playlist"]))?;
        for (index, entry) in playlist
            .as_array()
            .ok_or(Error::Protocol)?
            .iter()
            .take(2)
            .enumerate()
        {
            if entry.get("playing").and_then(Value::as_bool) == Some(true) {
                seen[index] = true;
            }
        }
        if pause_control && !paused && seen[0] && start.elapsed() >= Duration::from_secs(1) {
            mpv.pause(true)?;
            std::thread::sleep(Duration::from_millis(150));
            mpv.pause(false)?;
            paused = true;
        }
        if mpv
            .command(json!(["get_property", "idle-active"]))?
            .as_bool()
            == Some(true)
            && seen == [true, true]
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300)); // Include output drain/tail in monitor.
    mpv.shutdown()?;
    let pcm = monitor.finish()?;
    if pcm.len() >= CAP {
        return Err(Error::Protocol);
    }
    let measured = energy(&pcm)?;
    assert!(
        (3500..=5000).contains(&measured.active_ms),
        "approximately four seconds of owned tone output must reach monitor"
    );
    if pause_control {
        assert!(
            paused && measured.longest_quiet_ms >= 100,
            "real pause positive control must be observable"
        );
    }
    Ok(
        json!({"mpv_version": version, "gapless_option": gapless, "pause_control": pause_control, "prefetch_playlist": actual_prefetch, "default_prefetch_playlist": default_prefetch, "load_scripts": scripts,
        "playlist_occurrences_seen": seen, "captured_frames": pcm.len(), "audio_span_ms": measured.span_ms,
        "active_windows_ms": measured.active_ms, "longest_quiet_run_ms": measured.longest_quiet_ms}),
    )
}

#[test]
#[ignore = "requires explicitly owned p01_transition PulseAudio sink/monitor, mpv and FFmpeg; not physical output"]
fn live_current_next_monitor_energy_measurements() {
    assert!(
        std::env::var("P01_CAPTURE_MONITOR").as_deref() == Ok("p01_transition.monitor"),
        "explicit owned monitor required"
    );
    assert!(
        std::env::var("PULSE_SINK").as_deref() == Ok("p01_transition"),
        "explicit owned output sink required"
    );
    assert!(
        std::env::var("PULSE_SERVER").is_ok_and(|value| value.starts_with("unix:")),
        "isolated owned Unix audio server required"
    );
    let output_path =
        std::env::var_os("P01_TRANSITION_OUTPUT").expect("unused evidence path required");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let wav = [
        directory.path().join("first.wav"),
        directory.path().join("second.wav"),
    ];
    for (path, frequency) in wav.iter().zip([750, 1500]) {
        write_tone(path, 2, frequency).unwrap();
    }
    let binary = std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into());
    let mut cases = Vec::new();
    for format in ["wav", "flac", "mp3"] {
        let paths = if format == "wav" {
            wav.clone()
        } else {
            let paths = [
                directory.path().join(format!("first.{format}")),
                directory.path().join(format!("second.{format}")),
            ];
            for (source, target) in wav.iter().zip(&paths) {
                let encoder = if format == "flac" {
                    "flac"
                } else {
                    "libmp3lame"
                };
                let mut encoder_child = ManagedChild(
                    child_command(Path::new("ffmpeg"))
                        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
                        .arg(source)
                        .args(["-c:a", encoder])
                        .arg(target)
                        .spawn()
                        .unwrap(),
                );
                wait_encoder(&mut encoder_child, Duration::from_secs(10)).unwrap();
            }
            paths
        };
        for prefetch in [false, true] {
            for trial in 0..3 {
                let mut result = capture_pair(Path::new(&binary), &paths, false, prefetch).unwrap();
                result["format"] = json!(format);
                result["trial"] = json!(trial);
                cases.push(result);
            }
        }
    }
    let mut control = capture_pair(Path::new(&binary), &wav, true, true).unwrap();
    control["format"] = json!("wav");
    control["trial"] = json!(0);
    cases.push(control);
    serde_json::to_writer_pretty(&mut output, &json!({"sample_rate_hz": RATE, "window_ms": 1, "quiet_rms_threshold": QUIET_RMS,
        "tone_seconds_each": 2, "cases": cases, "scope": "Live owned PulseAudio monitor energy, 1 ms windows; not sample-exact gapless, physical/device output or crossfade certification"})).unwrap();
    output.write_all(b"\n").unwrap();
}

#[path = "crossfade.rs"]
mod crossfade;
