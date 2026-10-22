use feishin_spike_model::*;
use std::{collections::BTreeSet, sync::Arc};

fn tracks(ids: &[usize]) -> DragSource {
    DragSource::Tracks(Arc::new(ids.iter().copied().collect()))
}

fn apply(
    data: &[Record],
    rows: &[ViewRow],
    source: DragSource,
    before: ViewRow,
    grouped: bool,
) -> Result<Vec<ViewRow>, ReorderError> {
    reorder_view(
        data,
        &Reorder {
            rows: Arc::new(rows.to_vec()),
            source,
            before,
            grouped,
        },
        || false,
    )
    .unwrap()
}

#[test]
fn multiple_tracks_follow_display_order_and_compose_without_mutating_metadata() {
    let data = records(8);
    let initial = build_view(
        &data,
        &Query {
            descending: true,
            ..Default::default()
        },
        || false,
    )
    .unwrap();
    let first = apply(&data, &initial, tracks(&[2, 5]), ViewRow::Track(0), false).unwrap();
    assert_eq!(first, [7, 6, 4, 3, 1, 5, 2, 0].map(ViewRow::Track));
    let second = apply(&data, &first, tracks(&[7, 2]), ViewRow::Track(4), false).unwrap();
    assert_eq!(second, [6, 7, 2, 4, 3, 1, 5, 0].map(ViewRow::Track));
    assert_eq!(
        data.iter().map(|r| r.id).collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
    assert!(data.iter().all(|r| r.album == 0));
    // Dropping onto a selected item never changes the relative order.
    assert_eq!(
        apply(&data, &second, tracks(&[7, 2]), ViewRow::Track(2), false).unwrap(),
        second
    );
}

#[test]
fn album_moves_keep_blocks_and_track_moves_cannot_change_membership() {
    let data = records(30);
    let initial = build_view(
        &data,
        &Query {
            grouped: true,
            ..Default::default()
        },
        || false,
    )
    .unwrap();
    let moved = apply(
        &data,
        &initial,
        DragSource::Album(2),
        ViewRow::Track(0),
        true,
    )
    .unwrap();
    assert_eq!(&moved[..11], &initial[22..]);
    assert_eq!(&moved[11..], &initial[..22]);
    let moved = apply(&data, &moved, tracks(&[2, 5]), ViewRow::Album(0), true).unwrap();
    assert_eq!(
        &moved[11..15],
        &[
            ViewRow::Album(0),
            ViewRow::Track(2),
            ViewRow::Track(5),
            ViewRow::Track(0)
        ]
    );
    assert_eq!(
        apply(&data, &moved, tracks(&[2, 5]), ViewRow::Track(10), true),
        Err(ReorderError::CrossAlbum)
    );
    assert_eq!(
        apply(&data, &moved, tracks(&[2, 12]), ViewRow::Track(5), true),
        Err(ReorderError::CrossAlbum)
    );
    assert_eq!(
        apply(
            &data,
            &moved,
            DragSource::Album(2),
            ViewRow::Track(21),
            true
        )
        .unwrap(),
        moved
    );
}

#[test]
fn filtered_hidden_selection_missing_rows_bad_snapshots_and_cancellation() {
    let data = records(30);
    let rows = [
        ViewRow::Album(0),
        ViewRow::Track(1),
        ViewRow::Track(2),
        ViewRow::Track(3),
    ];
    // A hidden selected track in another album is not brought into this view.
    assert_eq!(
        apply(&data, &rows, tracks(&[1, 20]), ViewRow::Track(3), true).unwrap(),
        [
            ViewRow::Album(0),
            ViewRow::Track(2),
            ViewRow::Track(1),
            ViewRow::Track(3)
        ]
    );
    assert_eq!(
        apply(&data, &rows, tracks(&[20]), ViewRow::Track(3), true),
        Err(ReorderError::ChangedView)
    );
    assert_eq!(
        apply(&data, &rows, tracks(&[1]), ViewRow::Track(99), true),
        Err(ReorderError::ChangedView)
    );
    for rows in [
        vec![ViewRow::Track(99)],
        vec![ViewRow::Track(1), ViewRow::Track(1)],
        vec![ViewRow::Album(2), ViewRow::Track(1)],
    ] {
        assert_eq!(
            apply(&data, &rows, tracks(&[1]), ViewRow::Track(1), true),
            Err(ReorderError::ChangedView)
        );
    }
    let request = Reorder {
        rows: Arc::new(rows.to_vec()),
        source: tracks(&[1]),
        before: ViewRow::Track(2),
        grouped: true,
    };
    assert!(reorder_view(&data, &request, || true).is_none());
}

#[test]
fn generated_move_traces_match_a_simple_vector_oracle() {
    let data = records(128);
    let mut oracle: Vec<_> = (0..128).collect();
    let mut seed = 19u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as usize
    };
    for _ in 0..300 {
        let selected: BTreeSet<_> = (0..1 + next() % 16).map(|_| next() % 128).collect();
        let target = next() % 128;
        let actual = apply(
            &data,
            &oracle
                .iter()
                .copied()
                .map(ViewRow::Track)
                .collect::<Vec<_>>(),
            DragSource::Tracks(Arc::new(selected.clone())),
            ViewRow::Track(target),
            false,
        )
        .unwrap();
        if !selected.contains(&target) {
            let moving: Vec<_> = oracle
                .iter()
                .copied()
                .filter(|id| selected.contains(id))
                .collect();
            oracle.retain(|id| !selected.contains(id));
            let index = oracle.iter().position(|id| *id == target).unwrap();
            oracle.splice(index..index, moving);
        }
        assert_eq!(
            actual,
            oracle
                .iter()
                .copied()
                .map(ViewRow::Track)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            oracle.iter().copied().collect::<BTreeSet<_>>(),
            (0..128).collect()
        );
    }
}

#[test]
fn large_selection_is_bounded_and_can_cancel_in_either_scan() {
    use std::cell::Cell;
    let data = records(MAX_RECORDS);
    let rows = Arc::new(build_view(&data, &Query::default(), || false).unwrap());
    let request = Reorder {
        rows: rows.clone(),
        source: DragSource::Tracks(Arc::new((0..MAX_RECORDS).collect())),
        before: ViewRow::Track(50_000),
        grouped: false,
    };
    assert_eq!(
        reorder_view(&data, &request, || false).unwrap().unwrap(),
        *rows
    );
    for stop_at in [3, 500] {
        let calls = Cell::new(0);
        assert!(
            reorder_view(&data, &request, || {
                calls.set(calls.get() + 1);
                calls.get() >= stop_at
            })
            .is_none()
        );
        assert_eq!(calls.get(), stop_at);
    }
}

#[test]
fn drag_selection_snapshot_is_shared_and_survives_later_selection_changes() {
    let data = records(8);
    let rows = Arc::new(build_view(&data, &Query::default(), || false).unwrap());
    let mut selection = Selection::default();
    selection.select(&rows, 1, false, false);
    selection.select(&rows, 3, true, false);
    let snapshot = selection.selected.clone();
    assert!(Arc::ptr_eq(&snapshot, &selection.selected));
    selection.select(&rows, 7, false, false);
    assert_eq!(selection.selected.iter().copied().collect::<Vec<_>>(), [7]);
    assert_eq!(snapshot.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
    let request = Reorder {
        rows,
        source: DragSource::Tracks(snapshot),
        before: ViewRow::Track(6),
        grouped: false,
    };
    assert_eq!(
        reorder_view(&data, &request, || false).unwrap().unwrap(),
        [0, 4, 5, 1, 2, 3, 6, 7].map(ViewRow::Track)
    );
}
