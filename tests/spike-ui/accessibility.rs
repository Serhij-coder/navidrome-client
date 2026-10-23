use super::*;

#[test]
fn table_announcements_follow_focus_range_and_boundary_without_moving_keyboard_focus() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, false);
    let mut time = 0.0;
    keyboard_tests::frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    let button = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, button.rect.center());
    let mut status_id = None;
    for (key, modifiers, expected) in [
        (
            egui::Key::ArrowDown,
            egui::Modifiers::NONE,
            "Table focus: Track 000000, Album 0. 1 selected.",
        ),
        (
            egui::Key::ArrowDown,
            egui::Modifiers::SHIFT,
            "Table focus: Track 000001, Album 0. 2 selected.",
        ),
        (
            egui::Key::End,
            egui::Modifiers::NONE,
            "Table focus: Track 099999, Album 9999. 1 selected.",
        ),
        (
            egui::Key::Home,
            egui::Modifiers::NONE,
            "Table focus: Track 000000, Album 0. 1 selected.",
        ),
    ] {
        keyboard_tests::key(&mut app, &ctx, &mut time, key, modifiers);
        time += 0.1;
        let output = ctx.run(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 760.0),
                )),
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        let update = output.platform_output.accesskit_update.unwrap();
        let statuses: Vec<_> = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == egui::accesskit::Role::Status)
            .collect();
        assert_eq!(statuses.len(), 1);
        if let Some(previous) = status_id {
            assert_eq!(statuses[0].0, previous);
        }
        status_id = Some(statuses[0].0);
        let node = &statuses[0].1;
        assert_eq!(node.label(), Some(expected));
        assert_eq!(node.live(), Some(egui::accesskit::Live::Polite));
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(button.id));
        assert!(app.rows_drawn < 50);
    }
}
