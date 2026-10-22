use super::*;
fn frame(
    app: &mut SpikeApp,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    *time += 0.1;
    let _ = ctx.run(
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
    );
}
fn click(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64, position: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            time,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
    }
}
fn down(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            time,
            vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }],
            modifiers,
        );
    }
}
#[test]
fn focused_table_handles_real_arrows_and_leaves_text_edit_input_alone() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    let button = app.test_keyboard.as_ref().unwrap().clone();
    click(&mut app, &ctx, &mut time, button.rect.center());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(button.id));
    down(&mut app, &ctx, &mut time, egui::Modifiers::NONE);
    assert_eq!(app.selection.focus, Some(0));
    down(&mut app, &ctx, &mut time, egui::Modifiers::SHIFT);
    assert_eq!(
        app.selection.selected.iter().copied().collect::<Vec<_>>(),
        [0, 1]
    );
    let filter = app.test_filter.as_ref().unwrap().clone();
    click(&mut app, &ctx, &mut time, filter.rect.center());
    down(&mut app, &ctx, &mut time, egui::Modifiers::NONE);
    assert_eq!(app.selection.focus, Some(1));
}
