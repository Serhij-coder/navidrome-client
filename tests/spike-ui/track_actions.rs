use super::*;

fn frame(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64) -> egui::FullOutput {
    input(app, ctx, time, vec![], egui::Modifiers::NONE)
}
fn input(
    app: &mut SpikeApp,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    *time += 0.1;
    ctx.run(
        egui::RawInput {
            time: Some(*time),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 760.0),
            )),
            events,
            modifiers,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}

fn key(
    app: &mut SpikeApp,
    ctx: &egui::Context,
    time: &mut f64,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    let event = |pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    let output = input(app, ctx, time, vec![event(true)], modifiers);
    input(app, ctx, time, vec![event(false)], modifiers);
    output
}

fn copied(output: &egui::FullOutput) -> Vec<&str> {
    output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn copy_button(output: &egui::FullOutput) -> Option<egui::accesskit::NodeId> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == egui::accesskit::Role::Button
                && node.label() == Some("Copy synthetic title"))
            .then_some(*id)
        })
}

#[test]
fn copying_uses_the_focused_track_and_preserves_range_selection() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, true);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time);
    let table = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::SHIFT,
    );
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    frame(&mut app, &ctx, &mut time);
    let output = key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert_eq!(copied(&output), ["Track 000001"]);
    assert_eq!(app.selection.selected.len(), 2);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(table.id));
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::End,
        egui::Modifiers::NONE,
    );
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    frame(&mut app, &ctx, &mut time);
    assert_eq!(
        copied(&key(
            &mut app,
            &ctx,
            &mut time,
            egui::Key::Enter,
            egui::Modifiers::NONE
        )),
        ["Track 000099"]
    );
    assert!(app.rows_drawn < 50);
}

#[test]
fn absent_pending_or_hidden_focus_cannot_open_a_track_action() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, true);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time);
    let table = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    app.pending = true;
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
    app.pending = false;
    app.rows = Arc::new(vec![ViewRow::Track(99)]);
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
}

#[test]
fn changed_view_cancels_the_action_and_outside_text_focus_is_preserved() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, true);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time);
    let table = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_some());
    let filter = app.test_filter.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, filter.rect.center());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(filter.id));
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(filter.id));
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    frame(&mut app, &ctx, &mut time);
    app.generation += 1;
    assert!(
        copied(&key(
            &mut app,
            &ctx,
            &mut time,
            egui::Key::Enter,
            egui::Modifiers::NONE
        ))
        .is_empty()
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
}

#[test]
fn reopened_menu_items_have_fresh_ids_without_retaining_widget_state() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, true);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time);
    let table = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    let mut ids = std::collections::HashSet::new();
    let mut warm_size = None;
    for index in 0..64 {
        key(
            &mut app,
            &ctx,
            &mut time,
            egui::Key::F10,
            egui::Modifiers::SHIFT,
        );
        let output = frame(&mut app, &ctx, &mut time);
        assert!(ids.insert(copy_button(&output).unwrap()));
        key(
            &mut app,
            &ctx,
            &mut time,
            egui::Key::Escape,
            egui::Modifiers::NONE,
        );
        assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
        let size = ctx.data(|data| data.len());
        if index == 3 {
            warm_size = Some(size);
        }
        if let Some(warm) = warm_size {
            assert!(
                size <= warm + 4,
                "menu IDs must not retain per-opening widget state"
            );
        }
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(table.id));
    }
}

#[test]
fn keyboard_track_action_opens_and_escape_returns_to_the_table() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new(&ctx, true);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time);
    let table = app.test_keyboard.as_ref().unwrap().clone();
    keyboard_tests::click(&mut app, &ctx, &mut time, table.rect.center());
    keyboard_tests::key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    keyboard_tests::key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    let output = frame(&mut app, &ctx, &mut time);
    assert!(
        copy_button(&output).is_some(),
        "focused track action must be reachable without a pointer"
    );
    keyboard_tests::key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::Escape,
        egui::Modifiers::NONE,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(table.id));
    assert_eq!(app.selection.focus, Some(0));
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert!(copy_button(&frame(&mut app, &ctx, &mut time)).is_some());
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(table.id));
}
