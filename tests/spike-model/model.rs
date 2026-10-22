use feishin_spike_model::*;

#[test]
fn large_view_identity_grouping_filter_cancel_and_move() {
    let data = records(100_000);
    let grouped = build_view(
        &data,
        &Query {
            grouped: true,
            ..Default::default()
        },
        || false,
    )
    .unwrap();
    assert_eq!(grouped.len(), 110_000);
    assert_eq!(
        &grouped[..3],
        &[ViewRow::Album(0), ViewRow::Track(0), ViewRow::Track(1)]
    );
    assert!(build_view(&data, &Query::default(), || true).is_none());
    let query = Query {
        text: "99999".into(),
        ..Default::default()
    };
    assert_eq!(
        build_view(&data, &query, || false).unwrap(),
        [ViewRow::Track(99999)]
    );
    let moved = build_view(
        &data,
        &Query {
            move_before: Some((0, 2)),
            ..Default::default()
        },
        || false,
    )
    .unwrap();
    assert_eq!(
        &moved[..3],
        &[ViewRow::Track(1), ViewRow::Track(0), ViewRow::Track(2)]
    );
    assert_eq!(records(usize::MAX).len(), MAX_RECORDS);
}

#[test]
fn selection_skips_groups_and_retains_track_identity_across_order_change() {
    let view = [
        ViewRow::Album(0),
        ViewRow::Track(1),
        ViewRow::Track(2),
        ViewRow::Album(1),
        ViewRow::Track(3),
    ];
    let mut selection = Selection::default();
    assert_eq!(selection.navigate(&view, true, false), Some(1));
    selection.select(&view, 3, true, false);
    assert_eq!(
        selection.selected.iter().copied().collect::<Vec<_>>(),
        [1, 2, 3]
    );
    selection.select(&view, 2, false, true);
    assert!(!selection.selected.contains(&2));
    let reversed: Vec<_> = view.into_iter().rev().collect();
    selection.navigate(&reversed, true, false);
    assert_eq!(selection.focus, Some(1));
    selection.select(&view, 99, false, false);
    assert_eq!(selection.focus, Some(1));
}

#[test]
fn analysis_rejects_bad_frames_and_resolves_tone() {
    assert!(Analysis::from_pcm(&[0.0; 4]).is_none());
    assert!(Analysis::from_pcm(&[f32::NAN; PCM_SAMPLES]).is_none());
    let pcm: Vec<_> = (0..PCM_SAMPLES)
        .map(|i| (std::f32::consts::TAU * 4.0 * i as f32 / PCM_SAMPLES as f32).sin())
        .collect();
    let analysis = Analysis::from_pcm(&pcm).unwrap();
    assert!((analysis.bands[3] - 1.0).abs() < 0.001);
    assert!(
        analysis
            .bands
            .iter()
            .enumerate()
            .all(|(i, band)| i == 3 || *band < 0.001)
    );
    assert!(css_report("body { display:none }").contains("inert"));
}

#[test]
fn sort_uses_metadata_and_grouping_keeps_albums_contiguous() {
    let mut data = records(3).as_ref().clone();
    data[0].title = "Zulu".into();
    data[0].album = 1;
    data[1].title = "Alpha".into();
    data[1].album = 2;
    data[2].title = "Bravo".into();
    data[2].album = 1;
    assert_eq!(
        build_view(&data, &Query::default(), || false).unwrap(),
        [ViewRow::Track(1), ViewRow::Track(2), ViewRow::Track(0)]
    );
    assert_eq!(
        build_view(
            &data,
            &Query {
                grouped: true,
                ..Default::default()
            },
            || false
        )
        .unwrap(),
        [
            ViewRow::Album(1),
            ViewRow::Track(2),
            ViewRow::Track(0),
            ViewRow::Album(2),
            ViewRow::Track(1)
        ]
    );
}
