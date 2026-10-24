//! RawInput feasibility checks, not OS IME or script shaping certification.
use super::*;
use keyboard_tests::{click, frame, key};

fn ime(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64, event: egui::ImeEvent) {
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::Ime(event)],
        egui::Modifiers::NONE,
    );
}

fn setup() -> (SpikeApp, egui::Context, f64) {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    let table = app.test_keyboard.as_ref().unwrap().rect.center();
    click(&mut app, &ctx, &mut time, table);
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    (app, ctx, time)
}

#[test]
fn filter_ime_replaces_preedit_commits_and_isolates_table_commands() {
    let (mut app, ctx, mut time) = setup();
    let filter = app.test_filter.as_ref().unwrap().clone();
    click(&mut app, &ctx, &mut time, filter.rect.center());
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("に".into()),
    );
    assert_eq!(app.query.text, "に");
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("にほん".into()),
    );
    assert_eq!(app.query.text, "にほん");
    for input in [
        egui::Key::ArrowDown,
        egui::Key::ArrowUp,
        egui::Key::Home,
        egui::Key::End,
    ] {
        key(&mut app, &ctx, &mut time, input, egui::Modifiers::NONE);
    }
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert_eq!(app.selection.focus, Some(0));
    assert!(app.track_actions.is_none());
    assert_eq!(ctx.memory(|m| m.focused()), Some(filter.id));
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Commit("日本".into()),
    );
    assert_eq!(app.query.text, "日本");
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Disabled);
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Paste(" العربية e\u{301}".into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.query.text, "日本 العربية e\u{301}");
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::A,
        egui::Modifiers::COMMAND,
    );
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Text("音楽".into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.query.text, "音楽");
}

#[test]
fn multiline_css_composition_cancel_paste_and_scale_preserve_state() {
    let (mut app, ctx, mut time) = setup();
    let header = app.test_text_section.as_ref().unwrap().rect.center();
    click(&mut app, &ctx, &mut time, header);
    // Let the collapsing animation expose the actual editor before clicking it.
    for _ in 0..4 {
        frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    }
    let css = app.test_css.as_ref().unwrap().clone();
    click(&mut app, &ctx, &mut time, css.rect.center());
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("かな".into()),
    );
    assert_eq!(app.css, "かな");
    for input in [egui::Key::ArrowDown, egui::Key::Home, egui::Key::End] {
        key(&mut app, &ctx, &mut time, input, egui::Modifiers::NONE);
    }
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::F10,
        egui::Modifiers::SHIFT,
    );
    assert_eq!(app.selection.focus, Some(0));
    assert!(app.track_actions.is_none());
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Commit("仮名".into()),
    );
    assert_eq!(app.css, "仮名");
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::A,
        egui::Modifiers::COMMAND,
    );
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("取消".into()),
    );
    assert_eq!(app.css, "取消");
    // The integration cancels a prediction with an empty preedit, then disables IME.
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit(String::new()),
    );
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Disabled);
    assert_eq!(app.css, "");
    let content = "/* 日本 العربية e\u{301} */\nbody {\n  color: red;\n}";
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Paste(content.into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, content);
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::A,
        egui::Modifiers::COMMAND,
    );
    let cursor = egui::TextEdit::load_state(&ctx, css.id)
        .unwrap()
        .cursor
        .char_range();
    assert_eq!(
        cursor.unwrap().sorted_cursors().map(|cursor| cursor.index),
        [0, content.chars().count()]
    );
    for (zoom, density) in [(0.75, 1.0), (1.5, 2.0), (2.0, 1.25), (1.0, 1.0)] {
        ctx.set_zoom_factor(zoom);
        for _ in 0..2 {
            time += 0.1;
            let mut input = egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1200.0),
                )),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(density);
            let _ = ctx.run(input, |ctx| app.show(ctx));
        }
        assert_eq!(app.css, content);
        assert_eq!(ctx.memory(|m| m.focused()), Some(css.id));
        assert_eq!(
            egui::TextEdit::load_state(&ctx, css.id)
                .unwrap()
                .cursor
                .char_range(),
            cursor
        );
        assert_eq!(app.selection.focus, Some(0));
        assert!(app.rows_drawn > 0 && app.rows_drawn < 100);
        assert!((app.max_row_height - ROW_HEIGHT).abs() < 0.1);
        assert!((ctx.pixels_per_point() - zoom * density).abs() < 0.01);
    }
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Text("/* replaced 音楽 */".into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, "/* replaced 音楽 */");
    assert_eq!(app.query.text, "");
}

fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn focus_css(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64) -> egui::Id {
    let header = app.test_text_section.as_ref().unwrap().rect.center();
    click(app, ctx, time, header);
    for _ in 0..4 {
        frame(app, ctx, time, vec![], egui::Modifiers::NONE);
    }
    let css = app.test_css.as_ref().unwrap().clone();
    click(app, ctx, time, css.rect.center());
    css.id
}

#[test]
fn ime_enabled_without_preedit_and_finished_composition_allow_boundary_keys() {
    for boundary in [egui::Key::Home, egui::Key::End] {
        for finish in [
            None,
            Some(egui::ImeEvent::Commit("日本".into())),
            Some(egui::ImeEvent::Disabled),
        ] {
            let (mut app, ctx, mut time) = setup();
            focus_css(&mut app, &ctx, &mut time);
            frame(
                &mut app,
                &ctx,
                &mut time,
                vec![egui::Event::Text("tail".into())],
                egui::Modifiers::NONE,
            );
            ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
            let mut events = Vec::new();
            let expected_content = match finish {
                Some(egui::ImeEvent::Commit(_)) => {
                    ime(
                        &mut app,
                        &ctx,
                        &mut time,
                        egui::ImeEvent::Preedit("にほん".into()),
                    );
                    "tail日本"
                }
                Some(egui::ImeEvent::Disabled) => {
                    ime(
                        &mut app,
                        &ctx,
                        &mut time,
                        egui::ImeEvent::Preedit("取消".into()),
                    );
                    events.push(egui::Event::Ime(egui::ImeEvent::Preedit(String::new())));
                    "tail"
                }
                _ => "tail",
            };
            if let Some(finish) = finish {
                events.push(egui::Event::Ime(finish));
            }
            events.extend([
                key_event(boundary, true),
                key_event(boundary, false),
                egui::Event::Text("X".into()),
            ]);
            frame(&mut app, &ctx, &mut time, events, egui::Modifiers::NONE);
            let expected = if boundary == egui::Key::Home {
                format!("X{expected_content}")
            } else {
                format!("{expected_content}X")
            };
            assert_eq!(app.css, expected);
            assert_eq!(app.selection.focus, Some(0));
        }
    }
}

#[test]
fn preedit_then_boundary_then_commit_in_one_frame_preserves_replacement() {
    let (mut app, ctx, mut time) = setup();
    focus_css(&mut app, &ctx, &mut time);
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![
            egui::Event::Ime(egui::ImeEvent::Enabled),
            egui::Event::Ime(egui::ImeEvent::Preedit("にほん".into())),
            egui::Event::Ime(egui::ImeEvent::Preedit("\n".into())),
            egui::Event::Ime(egui::ImeEvent::Commit("\r".into())),
            key_event(egui::Key::Home, true),
            key_event(egui::Key::Home, false),
            egui::Event::Ime(egui::ImeEvent::Commit("日本".into())),
            key_event(egui::Key::End, true),
            key_event(egui::Key::End, false),
            egui::Event::Text("X".into()),
        ],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, "日本X");
}

#[test]
fn pointer_focus_transfer_drops_stale_composition_before_table_boundary_key() {
    let (mut app, ctx, mut time) = setup();
    let css_id = focus_css(&mut app, &ctx, &mut time);
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("かな".into()),
    );
    let table = app.test_keyboard.as_ref().unwrap().clone();
    let pos = table.rect.center();
    let mut events = vec![egui::Event::PointerMoved(pos)];
    for pressed in [true, false] {
        events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    events.extend([
        key_event(egui::Key::End, true),
        key_event(egui::Key::End, false),
    ]);
    frame(&mut app, &ctx, &mut time, events, egui::Modifiers::NONE);
    assert_eq!(ctx.memory(|m| m.focused()), Some(table.id));
    assert_eq!(app.selection.focus, Some(99_999));
    assert!(app.track_actions.is_none());
    let css_pos = app.test_css.as_ref().unwrap().rect.center();
    click(&mut app, &ctx, &mut time, css_pos);
    assert_eq!(ctx.memory(|m| m.focused()), Some(css_id));
    key(
        &mut app,
        &ctx,
        &mut time,
        egui::Key::Home,
        egui::Modifiers::NONE,
    );
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Text("X".into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, "Xかな");
}

#[test]
fn empty_preedit_cancels_guard_without_waiting_for_disabled() {
    let (mut app, ctx, mut time) = setup();
    focus_css(&mut app, &ctx, &mut time);
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![egui::Event::Text("tail".into())],
        egui::Modifiers::NONE,
    );
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("取消".into()),
    );
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit(String::new())),
            key_event(egui::Key::Home, true),
            key_event(egui::Key::Home, false),
            egui::Event::Text("X".into()),
        ],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, "Xtail");
}

#[test]
fn window_focus_loss_drops_composition_guard_without_disabled_event() {
    let (mut app, ctx, mut time) = setup();
    let css_id = focus_css(&mut app, &ctx, &mut time);
    ime(&mut app, &ctx, &mut time, egui::ImeEvent::Enabled);
    ime(
        &mut app,
        &ctx,
        &mut time,
        egui::ImeEvent::Preedit("かな".into()),
    );
    time += 0.1;
    let _ = ctx.run(
        egui::RawInput {
            time: Some(time),
            focused: false,
            events: vec![egui::Event::WindowFocused(false)],
            ..Default::default()
        },
        |ctx| app.show(ctx),
    );
    // Reacquire the editor as an integration would after the window returns.
    ctx.memory_mut(|memory| memory.request_focus(css_id));
    frame(
        &mut app,
        &ctx,
        &mut time,
        vec![
            egui::Event::WindowFocused(true),
            key_event(egui::Key::Home, true),
            key_event(egui::Key::Home, false),
            egui::Event::Text("X".into()),
        ],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.css, "Xかな");
}
