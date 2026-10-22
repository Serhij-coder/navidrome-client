use super::keyboard_tests::{click, frame};
use super::*;

fn position(app: &SpikeApp, row: ViewRow) -> egui::Pos2 {
    app.test_rows
        .iter()
        .find(|(id, _)| *id == row)
        .expect("test row must be in viewport")
        .1
        .rect
        .center()
}

fn settle(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while app.pending {
        assert!(std::time::Instant::now() < deadline, "view worker deadline");
        std::thread::sleep(Duration::from_millis(2));
        frame(app, ctx, time, vec![], egui::Modifiers::NONE);
    }
}

fn drag(app: &mut SpikeApp, ctx: &egui::Context, time: &mut f64, source: ViewRow, target: ViewRow) {
    let from = position(app, source);
    let to = position(app, target);
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::PointerMoved(from)],
        egui::Modifiers::NONE,
    );
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    // Begin dragging away from the source before reaching the target.
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::PointerMoved(from + egui::vec2(10.0, 0.0))],
        egui::Modifiers::NONE,
    );
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::PointerMoved(to)],
        egui::Modifiers::NONE,
    );
    frame(
        app,
        ctx,
        time,
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    settle(app, ctx, time);
}

#[test]
fn real_pointer_multi_drag_retains_selection_and_composes_moves() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    let first = position(&app, ViewRow::Track(0));
    click(&mut app, &ctx, &mut time, first);
    let second = position(&app, ViewRow::Track(2));
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            &mut time,
            vec![
                egui::Event::PointerMoved(second),
                egui::Event::PointerButton {
                    pos: second,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            egui::Modifiers::COMMAND,
        );
    }
    assert_eq!(
        app.selection.selected.iter().copied().collect::<Vec<_>>(),
        [0, 2]
    );
    drag(
        &mut app,
        &ctx,
        &mut time,
        ViewRow::Track(0),
        ViewRow::Track(5),
    );
    assert_eq!(&app.rows[..6], &[1, 3, 4, 0, 2, 5].map(ViewRow::Track));
    assert_eq!(
        app.selection.selected.iter().copied().collect::<Vec<_>>(),
        [0, 2]
    );
    drag(
        &mut app,
        &ctx,
        &mut time,
        ViewRow::Track(1),
        ViewRow::Track(4),
    );
    assert_eq!(&app.rows[..6], &[3, 1, 4, 0, 2, 5].map(ViewRow::Track));
    assert!(app.rows_drawn < 50);
    app.submit(&ctx); // Sorting/filtering/reset rebuild from metadata.
    settle(&mut app, &ctx, &mut time);
    assert_eq!(&app.rows[..6], &[0, 1, 2, 3, 4, 5].map(ViewRow::Track));
}

#[test]
fn real_pointer_album_drag_reorders_whole_blocks_and_rejects_cross_album_tracks() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, false);
    app.query.grouped = true;
    app.rows = Arc::new(build_view(&app.data, &app.query, || false).unwrap());
    let mut time = 0.0;
    frame(&mut app, &ctx, &mut time, vec![], egui::Modifiers::NONE);
    drag(
        &mut app,
        &ctx,
        &mut time,
        ViewRow::Album(1),
        ViewRow::Album(0),
    );
    assert_eq!(
        &app.rows[..3],
        &[ViewRow::Album(1), ViewRow::Track(10), ViewRow::Track(11)]
    );
    assert_eq!(app.rows[11], ViewRow::Album(0));
    let before = app.rows.clone();
    drag(
        &mut app,
        &ctx,
        &mut time,
        ViewRow::Track(10),
        ViewRow::Album(0),
    );
    assert_eq!(*app.rows, *before);
    assert_eq!(app.reorder_notice, Some(ReorderError::CrossAlbum.message()));
    assert!(app.rows_drawn < 50);
}

#[test]
fn pending_or_replaced_view_rejects_an_obsolete_drag() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, true);
    let source = RowDrag {
        source: DragSource::Album(0),
        rows: app.rows.clone(),
    };
    app.pending = true;
    app.reorder(&ctx, &source, ViewRow::Track(10));
    assert_eq!(app.generation, 0);
    app.pending = false;
    app.rows = Arc::new(app.rows.as_ref().clone());
    app.reorder(&ctx, &source, ViewRow::Track(10));
    assert_eq!(app.generation, 0);
    assert_eq!(
        app.reorder_notice,
        Some(ReorderError::ChangedView.message())
    );
}
