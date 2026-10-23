//! Owned-monitor calibration and live two-mpv feasibility, not production mixing.
use super::*;

const SPECTRAL_WINDOW: usize = RATE / 10;
fn amplitude(pcm: &[f32], frequency: f64) -> Result<f64, Error> {
    if pcm.is_empty() || pcm.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::Protocol);
    }
    let (mut real, mut imag) = (0.0, 0.0);
    for (index, sample) in pcm.iter().enumerate() {
        let angle = std::f64::consts::TAU * frequency * index as f64 / RATE as f64;
        real += f64::from(*sample) * angle.cos();
        imag -= f64::from(*sample) * angle.sin();
    }
    Ok(2.0 * real.hypot(imag) / pcm.len() as f64)
}
fn mean_amplitude(pcm: &[f32], start: usize, end: usize, frequency: f64) -> Result<f64, Error> {
    let region = pcm.get(start..end).ok_or(Error::Protocol)?;
    let values: Vec<_> = region
        .chunks_exact(SPECTRAL_WINDOW)
        .map(|chunk| amplitude(chunk, frequency))
        .collect::<Result<_, _>>()?;
    if values.is_empty() {
        return Err(Error::Protocol);
    }
    Ok(values.iter().sum::<f64>() / values.len() as f64)
}
fn consent() {
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
        "owned Unix server required"
    );
}

#[test]
fn spectral_projection_separates_owned_tones_and_rejects_missing_data() {
    let pcm: Vec<_> = (0..SPECTRAL_WINDOW)
        .map(|sample| {
            let t = sample as f64 / RATE as f64;
            (0.2 * (std::f64::consts::TAU * 750.0 * t).sin()
                + 0.1 * (std::f64::consts::TAU * 1500.0 * t).sin()) as f32
        })
        .collect();
    assert!((amplitude(&pcm, 750.0).unwrap() - 0.2).abs() < 1e-7);
    assert!((amplitude(&pcm, 1500.0).unwrap() - 0.1).abs() < 1e-7);
    assert!(amplitude(&[], 750.0).is_err());
    assert!(amplitude(&[f32::NAN], 750.0).is_err());
    assert!(mean_amplitude(&pcm, 0, pcm.len() + 1, 750.0).is_err());
}

#[test]
#[ignore = "requires explicitly owned p01_transition sink/monitor, mpv and FFmpeg; no physical audio"]
fn live_mpv_volume_calibration() {
    consent();
    let output_path = std::env::var_os("P01_GAIN_OUTPUT").expect("new evidence path required");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 12, 750).unwrap();
    let binary = std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into());
    let mut monitor = Monitor::start_with_cap(RATE * 20).unwrap();
    monitor.ready().unwrap();
    let mut mpv = Mpv::start(Path::new(&binary), 1, AudioOutput::Pulse).unwrap();
    mpv.pause(true).unwrap();
    mpv.load_local(&tone, false).unwrap();
    mpv.set_volume_percent(100.0).unwrap();
    mpv.pause(false).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    let version = mpv.command(json!(["get_property", "mpv-version"])).unwrap();
    let mut marks = Vec::new();
    for percent in [100.0, 50.0, 25.0, 0.0, 100.0] {
        mpv.set_volume_percent(percent).unwrap();
        let frame = monitor.frames.load(Ordering::Acquire);
        marks.push((percent, frame));
        std::thread::sleep(Duration::from_millis(1300));
    }
    mpv.shutdown().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let pcm = monitor.finish().unwrap();
    assert!(pcm.len() < RATE * 20 && pcm.iter().all(|sample| sample.is_finite()));
    let values:Vec<_> = marks.into_iter().map(|(percent,frame)| {
        json!({"percent":percent,"frame_after_command":frame,"amplitude":mean_amplitude(&pcm,frame+RATE/2,frame+RATE,750.0).unwrap()})
    }).collect();
    assert!(values[0]["amplitude"].as_f64().unwrap() > 0.1);
    assert!(values[3]["amplitude"].as_f64().unwrap() < 0.002);
    let base = values[0]["amplitude"].as_f64().unwrap();
    assert!(
        (values[1]["amplitude"].as_f64().unwrap() / base - 0.125).abs() < 0.005,
        "50 percent must confirm the assumed cubic law"
    );
    assert!(
        (values[2]["amplitude"].as_f64().unwrap() / base - 0.015625).abs() < 0.002,
        "25 percent must confirm the assumed cubic law"
    );
    assert!(
        (values[4]["amplitude"].as_f64().unwrap() / base - 1.0).abs() < 0.03,
        "restored full-volume baseline must be stable"
    );
    serde_json::to_writer_pretty(&mut output,&json!({"rate_hz":RATE,"window_ms":100,"mpv_version":version,"cases":values,
        "scope":"Owned monitor amplitude calibration at fixed mpv percentage plateaus; no device/human output or crossfade claim"})).unwrap();
    output.write_all(b"\n").unwrap();
}

struct Pair {
    outgoing: Mpv,
    incoming: Mpv,
    generation: u64,
}
impl Pair {
    fn apply(
        &mut self,
        curve: feishin_spike_model::FadeCurve,
        progress: f64,
        generation: u64,
    ) -> Result<(), Error> {
        if generation != self.generation {
            return Err(Error::InvalidInput);
        }
        if !self.outgoing.accepts_generation(generation)
            || !self.incoming.accepts_generation(generation)
        {
            self.close()?;
            return Err(Error::ProcessExited);
        }
        let (outgoing, incoming) = curve
            .web_gain_scalars(progress, 1.0)
            .ok_or(Error::InvalidInput)?;
        // Observed mpv 0.37 percentage -> amplitude law is cubic. The source
        // browser engine assigns squared slider coefficients; convert scalars
        // explicitly rather than assuming mpv percentages are amplitude gains.
        let result = self
            .outgoing
            .set_volume_percent(100.0 * outgoing.cbrt())
            .and_then(|()| self.incoming.set_volume_percent(100.0 * incoming.cbrt()));
        if let Err(error) = result {
            self.close()?;
            return Err(error);
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        let outgoing = self.outgoing.shutdown();
        let incoming = self.incoming.shutdown();
        outgoing.and(incoming)
    }
}
fn ready_position(player: &mut Mpv) -> Result<f64, Error> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(Error::Deadline)?;
        let reply = player.probe_position(remaining)?;
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        if let Some(position) = reply {
            return Ok(position);
        }
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pair_rejects_stale_work_and_stops_both_players_after_peer_death() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("normal");
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_mpv.py"
        ),
        &executable,
    )
    .unwrap();
    let mut pair = Pair {
        outgoing: Mpv::start(&executable, 31, AudioOutput::Null).unwrap(),
        incoming: Mpv::start(&executable, 31, AudioOutput::Null).unwrap(),
        generation: 31,
    };
    assert!(matches!(
        pair.apply(feishin_spike_model::FadeCurve::Linear, 0.5, 30),
        Err(Error::InvalidInput)
    ));
    assert!(pair.outgoing.accepts_generation(31) && pair.incoming.accepts_generation(31));
    pair.incoming.terminate_for_probe().unwrap();
    assert!(matches!(
        pair.apply(feishin_spike_model::FadeCurve::Linear, 0.5, 31),
        Err(Error::ProcessExited)
    ));
    assert!(!pair.outgoing.accepts_generation(31) && !pair.incoming.accepts_generation(31));
}

fn live_case(
    binary: &Path,
    paths: &[std::path::PathBuf; 2],
    curve: feishin_spike_model::FadeCurve,
) -> Result<Value, Error> {
    let mut monitor = Monitor::start_with_cap(RATE * 24)?;
    monitor.ready()?;
    let mut pair = Pair {
        outgoing: Mpv::start(binary, 4, AudioOutput::Pulse)?,
        incoming: Mpv::start(binary, 4, AudioOutput::Pulse)?,
        generation: 4,
    };
    pair.outgoing.pause(true)?;
    pair.incoming.pause(true)?;
    pair.outgoing.load_local(&paths[0], false)?;
    pair.incoming.load_local(&paths[1], false)?;
    pair.apply(curve, 0.0, 4)?;
    ready_position(&mut pair.outgoing)?;
    ready_position(&mut pair.incoming)?;
    pair.outgoing.pause(false)?;
    std::thread::sleep(Duration::from_millis(1600));
    let head_frame = monitor.frames.load(Ordering::Acquire);
    let start_position = ready_position(&mut pair.outgoing)?;
    pair.incoming.pause(false)?;
    let started = Instant::now();
    let mut marks = Vec::new();
    let mut midpoint = None;
    loop {
        if started.elapsed() > Duration::from_secs(7) {
            return Err(Error::Deadline);
        }
        let position = pair.outgoing.position()?;
        let progress = ((position - start_position) / 3.0).clamp(0.0, 1.0);
        pair.apply(curve, progress, 4)?;
        if started.elapsed() > Duration::from_secs(7) {
            return Err(Error::Deadline);
        }
        let frame = monitor.frames.load(Ordering::Acquire);
        let (out_gain, in_gain) = curve
            .web_gain_scalars(progress, 1.0)
            .ok_or(Error::InvalidInput)?;
        marks.push(json!({"elapsed_ms":started.elapsed().as_millis(),"media_progress":progress,"capture_frame_after_commands":frame,"outgoing_gain_scalar":out_gain,"incoming_gain_scalar":in_gain}));
        if midpoint.is_none() && progress >= 0.5 {
            midpoint = Some(frame);
        }
        if progress >= 1.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    pair.outgoing.pause(true)?;
    let tail_frame = monitor.frames.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(1400));
    pair.close()?;
    let pcm = monitor.finish()?;
    if pcm.len() >= RATE * 24 || pcm.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::Protocol);
    }
    let head_start = head_frame.saturating_sub(RATE / 2);
    let out_base = mean_amplitude(&pcm, head_start, head_frame, 750.0)?;
    let in_base = mean_amplitude(&pcm, tail_frame + RATE / 2, tail_frame + RATE, 1500.0)?;
    let mid = midpoint.ok_or(Error::Protocol)? + RATE / 10;
    let out_mid = mean_amplitude(&pcm, mid, mid + RATE / 10, 750.0)? / out_base;
    let in_mid = mean_amplitude(&pcm, mid, mid + RATE / 10, 1500.0)? / in_base;
    let out_tail =
        mean_amplitude(&pcm, tail_frame + RATE / 2, tail_frame + RATE, 750.0)? / out_base;
    let in_head = mean_amplitude(&pcm, head_start, head_frame, 1500.0)? / in_base;
    assert!(
        out_base > 0.1 && in_base > 0.1,
        "both owned tones need stable baselines"
    );
    assert!(
        in_head < 0.02 && out_tail < 0.02,
        "muted endpoints must suppress the other source"
    );
    assert!(
        (0.05..0.85).contains(&out_mid) && (0.05..0.85).contains(&in_mid),
        "both sources must be present in the captured middle window"
    );
    let peak = pcm
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!(peak < 0.95, "owned fixture mix must retain headroom");
    let spectrum:Vec<_>=pcm.chunks_exact(SPECTRAL_WINDOW).enumerate().map(|(index,window)|Ok(json!({"capture_ms":index*100,"outgoing_relative":amplitude(window,750.0)?/out_base,"incoming_relative":amplitude(window,1500.0)?/in_base}))).collect::<Result<_,Error>>()?;
    Ok(
        json!({"curve":format!("{curve:?}"),"fade_media_seconds":3,"master_fraction":1,"speed":1,"captured_frames":pcm.len(),"capture_head_frame":head_frame,"capture_tail_frame":tail_frame,"outgoing_baseline":out_base,"incoming_baseline":in_base,"outgoing_mid_relative":out_mid,"incoming_mid_relative":in_mid,"outgoing_tail_relative":out_tail,"incoming_head_relative":in_head,"peak":peak,"commands":marks,"spectral_windows":spectrum}),
    )
}

#[test]
#[ignore = "requires explicit owned p01_transition sink/monitor, mpv and FFmpeg; not device or production crossfade"]
fn live_two_mpv_crossfade() {
    consent();
    let output_path = std::env::var_os("P01_CROSSFADE_OUTPUT").expect("new evidence path required");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let paths = [
        directory.path().join("outgoing.wav"),
        directory.path().join("incoming.wav"),
    ];
    for (path, frequency) in paths.iter().zip([750, 1500]) {
        write_tone(path, 12, frequency).unwrap();
    }
    let binary = std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into());
    let mut cases = Vec::new();
    for curve in [
        feishin_spike_model::FadeCurve::Linear,
        feishin_spike_model::FadeCurve::EqualPower,
        feishin_spike_model::FadeCurve::SCurve,
    ] {
        cases.push(live_case(Path::new(&binary), &paths, curve).unwrap());
    }
    serde_json::to_writer_pretty(&mut output,&json!({"rate_hz":RATE,"spectral_window_ms":100,"cases":cases,
        "scope":"Live two-owned-mpv slider-law conversion and captured two-tone overlap; no full waveform/timestamp, audible output, production state owner, speed/codec/platform or fallback certification"})).unwrap();
    output.write_all(b"\n").unwrap();
}

#[test]
fn partial_gain_failure_stops_and_reaps_both_players() {
    let directory = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for mode in ["normal", "malformed"] {
        let path = directory.path().join(mode);
        std::os::unix::fs::symlink(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native-spike/fake_mpv.py"
            ),
            &path,
        )
        .unwrap();
        paths.push(path);
    }
    let mut pair = Pair {
        outgoing: Mpv::start(&paths[0], 22, AudioOutput::Null).unwrap(),
        incoming: Mpv::start(&paths[1], 22, AudioOutput::Null).unwrap(),
        generation: 22,
    };
    assert!(matches!(
        pair.apply(feishin_spike_model::FadeCurve::EqualPower, 0.5, 22),
        Err(Error::Protocol)
    ));
    assert!(!pair.outgoing.accepts_generation(22) && !pair.incoming.accepts_generation(22));
    assert!(pair.outgoing.child.try_wait().unwrap().is_some());
    assert!(pair.incoming.child.try_wait().unwrap().is_some());
}
