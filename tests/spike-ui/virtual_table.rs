use feishin_spike_ui::SpikeApp;
#[test]
fn hundred_thousand_rows_draw_only_viewport_and_idle_has_no_periodic_repaint() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    for frame in 0..12 {
        let output = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 * 0.1),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        assert!(
            app.rows_drawn > 0 && app.rows_drawn < 50,
            "{} rows",
            app.rows_drawn
        );
        assert!(
            app.max_row_height <= feishin_spike_ui::ROW_HEIGHT + 1.0,
            "rows exceed fixed virtual stride: {}",
            app.max_row_height
        );
        if frame == 11 {
            assert_eq!(
                output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                std::time::Duration::MAX
            );
        }
    }
}

#[test]
fn artwork_cells_preserve_virtual_row_height() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    app.set_artwork_visible(true);
    for frame in 0..16 {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 / 60.0),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        assert!(app.max_row_height <= feishin_spike_ui::ROW_HEIGHT + 1.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
