//! Explicit lab experiment; private access drives the real worker, not a mock view.
use super::*;
use feishin_native_spike::{AnalysisSession, AnalysisSource, AudioOutput, Mpv, write_tone};
use std::{io::Write, time::Instant};

#[test]
#[ignore = "release lab probe needs explicit owned PulseAudio monitor, mpv/FFmpeg and unused output path; not a presented-frame acceptance benchmark"]
fn save_scrolling_worker_artwork_playback_samples() {
    assert_eq!(
        std::env::var("P01_CAPTURE_MONITOR").as_deref(),
        Ok("p01_owned.monitor")
    );
    let output =
        std::env::var_os("P01_CONCURRENT_OUTPUT").expect("set unused P01_CONCURRENT_OUTPUT");
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 30, 750).unwrap();
    let mpv = std::env::var_os("P01_MPV").expect("set trusted P01_MPV");
    let mut playback = Mpv::start(std::path::Path::new(&mpv), 1, AudioOutput::Pulse).unwrap();
    playback.load_local(&tone, false).unwrap();
    let mut capture = AnalysisSession::new(
        "ffmpeg".into(),
        AnalysisSource::Monitor("p01_owned.monitor".into()),
        || {},
    )
    .unwrap();
    capture.set_active(true);
    let source_started = Instant::now();
    loop {
        if capture
            .latest()
            .is_some_and(|frame| frame.bands.iter().any(|band| *band > 0.01))
        {
            break;
        }
        assert!(
            source_started.elapsed() < Duration::from_secs(6),
            "owned source not ready: {:?}",
            capture.status()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let source_ready_ms = source_started.elapsed().as_secs_f64() * 1000.0;
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    let mut data = records(100_000).as_ref().clone();
    for record in &mut data {
        record.title = format!("Track {:06}", (record.id * 7919) % 100_000);
        record.album = (record.id * 31) % 10_000;
    }
    app.data = Arc::new(data);
    app.worker = worker::Worker::new(app.data.clone(), ctx.clone());
    app.rows = Arc::new(build_view(&app.data, &Query::default(), || false).unwrap());
    app.set_artwork_visible(true);
    app.enable_analysis();
    let mut frames = Vec::new();
    let mut completions = Vec::new();
    let mut positions = Vec::new();
    let mut submitted = None;
    let mut analysis_frames = 0;
    let mut max_rows = 0;
    let mut last_input_frame = None;
    for frame in 0..6020 {
        if frame % 300 == 0 {
            app.query.descending = !app.query.descending;
            app.query.grouped = frame % 600 == 0;
            app.query.text = if frame % 900 == 0 {
                "1".into()
            } else {
                String::new()
            };
            app.submit(&ctx);
            submitted = Some((app.generation, Instant::now()));
        }
        if let Some(analysis) = capture.latest() {
            assert!(analysis.bands.iter().any(|band| *band > 0.01));
            app.analysis = Some(analysis);
            analysis_frames += 1;
        }
        if frame % 600 == 0 {
            positions.push(playback.position().unwrap());
        }
        let mut events = vec![
            egui::Event::PointerMoved(egui::pos2(400.0, 400.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, if frame % 1200 < 600 { -400.0 } else { 400.0 }),
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if frame % 60 == 30 {
            app.test_keyboard.as_ref().unwrap().request_focus();
            last_input_frame = Some(frame);
            events.push(egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        } else if last_input_frame == Some(frame - 1) {
            events.push(egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let start = Instant::now();
        let output = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 / 60.0),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 760.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        std::hint::black_box(output);
        if frame >= 20 {
            frames.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        max_rows = max_rows.max(app.rows_drawn);
        assert!(app.rows_drawn < 50);
        if !app.pending
            && let Some((generation, start)) = submitted.take()
        {
            completions.push(serde_json::json!({"generation":generation,"completion_ms":start.elapsed().as_secs_f64()*1000.0}));
        }
        // Yield enough wall time for actual decode/playback; simulated UI time is separate.
        if frame % 10 == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert!(analysis_frames > 0, "no owned monitor frames");
    assert!(
        positions.last().unwrap() > positions.first().unwrap(),
        "mpv clock did not progress"
    );
    capture.shutdown().unwrap();
    playback.shutdown().unwrap();
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")
                    .map(str::trim)
                    .map(str::to_owned)
            })
        });
    let report = serde_json::json!({"scope":"P01 CPU-only egui layout while synthetic scroll/key events, real worker sort/filter/group, bounded owned PNG decode, mpv owned-tone playback and monitor analysis run concurrently; no presented frames, hardware input, total helper RSS, GPU, minimized/tray or P00 budget acceptance",
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"}, "layout_ms":frames,
        "view_completions":completions,"max_rows_drawn":max_rows,"analysis_frames_consumed":analysis_frames,"source_ready_ms":source_ready_ms,
        "mpv_position_seconds":positions,"owner_peak_rss_linux":rss,
        "fixture":"100k records; title permutation (id*7919)%100000, noncontiguous albums (id*31)%10000; one original PNG",
        "host_conditions":"development host; uncalibrated; CPU governor, thermals and background load uncontrolled"});
    let mut file = std::fs::File::create_new(output).unwrap();
    file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .unwrap();
    file.write_all(b"\n").unwrap();
}
