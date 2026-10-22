use feishin_spike_model::{Query, build_view, records};
use feishin_spike_ui::SpikeApp;
use std::{io::Write, time::Instant};

#[test]
#[ignore = "manual release benchmark; CPU-only layout, not presented frames or acceptance budgets"]
fn save_raw_headless_samples() {
    let output = std::env::var_os("P01_BENCH_OUTPUT").expect("set an unused P01_BENCH_OUTPUT path");
    let mut cases = Vec::new();
    for size in [10_000, 100_000] {
        let records = records(size);
        for (name, query) in [
            (
                "reverse",
                Query {
                    descending: true,
                    ..Default::default()
                },
            ),
            (
                "filter",
                Query {
                    text: "123".into(),
                    ..Default::default()
                },
            ),
            (
                "group",
                Query {
                    grouped: true,
                    ..Default::default()
                },
            ),
        ] {
            let mut samples = Vec::new();
            for _ in 0..31 {
                let start = Instant::now();
                let view = build_view(&records, &query, || false).unwrap();
                std::hint::black_box(view);
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.remove(0);
            cases.push(serde_json::json!({"workload":name, "records":size, "samples_ms":samples}));
        }
    }
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    let mut frames = Vec::new();
    let mut max_rows = 0;
    for frame in 0..6010 {
        let start = Instant::now();
        let output = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 / 60.0),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 760.0),
                )),
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        std::hint::black_box(output);
        if frame >= 10 {
            frames.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        max_rows = max_rows.max(app.rows_drawn);
    }
    let report = serde_json::json!({
        "scope":"P01 CPU-only synthetic view construction and stationary egui layout; not GPU/presented frame, scrolling, playback, artwork, OS input latency or P00 budget acceptance",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "view_cases":cases, "stationary_layout_ms":frames, "max_rows_drawn":max_rows,
        "fixture":"records(count), monotonic synthetic titles, 10 songs per album", "viewport_points":[1100,760]
    });
    let mut file = std::fs::File::create_new(output).unwrap();
    file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .unwrap();
    file.write_all(b"\n").unwrap();
}
