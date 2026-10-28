use feishin_storage::{MAX_BYTES, imports::*};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const OLD: &[u8] = include_bytes!("../fixtures/p02/imports/settings-v14.json");
const PLAYER3: &[u8] = include_bytes!("../fixtures/p02/imports/player-v3.json");
const PLAYER4: &[u8] = include_bytes!("../fixtures/p02/imports/player-v4.json");
const QUEUE: &[u8] = include_bytes!("../fixtures/p02/imports/queue-v4.json");
const AUTH: &[u8] = include_bytes!("../fixtures/p02/imports/auth-v2.json");
fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn has(preview: &ImportPreview, code: Code) -> bool {
    preview.report().diagnostics.iter().any(|d| d.code == code)
}
fn column(id: &str) -> Value {
    json!({"id":id,"align":"start","autoSize":false,"isEnabled":true,"pinned":null,"width":100})
}
fn current_settings(value: &Value) -> Vec<u8> {
    bytes(&json!({"version":34,"state":value}))
}

#[test]
fn actual_saved_v34_export_projects_supported_subset_without_mutating_copy() {
    let captured =
        include_bytes!("../fixtures/p00/browser/2026-10-22/navidrome-current-settings-export.json");
    let before = captured.to_vec();
    let imported = preview(
        &CopiedInputs {
            settings: Some(captured),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(captured.as_slice(), before);
    assert!(imported.report().partial);
    assert_eq!(imported.data().profile.preferences.language, "en");
    assert!(!imported.data().profile.preferences.follow_system_theme);
    assert_eq!(
        imported.data().auto_dj.as_ref().unwrap().mode,
        AutoDjMode::Songs
    );
    assert!(imported.data().columns.len() >= 12);
    assert!(
        imported
            .data()
            .columns
            .iter()
            .any(|c| c.list == ListId::Album && c.view == ColumnView::Detail)
    );
    assert!(has(&imported, Code::UnsupportedCategory));
    assert!(!has(&imported, Code::CorruptCategory));
    assert!(!has(&imported, Code::InvalidColumns));
    let serialized = serde_json::to_string(imported.data()).unwrap();
    assert!(!serialized.contains("password"));
    assert!(serialized.len() < MAX_BYTES);
}
#[test]
fn source_derived_historical_combined_projection_preserves_autodj_and_reports_deferrals() {
    let imported = preview(
        &CopiedInputs {
            settings: Some(OLD),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(has(&imported, Code::DeferredMigrations));
    assert_eq!(imported.data().profile.preferences.language, "uk");
    let auto = imported.data().auto_dj.as_ref().unwrap();
    assert_eq!(auto.album_strategy, AutoDjStrategy::LibraryRandom);
    assert_eq!(auto.mode, AutoDjMode::Albums);
    assert_eq!(auto.item_count.as_f64(), Some(7.5));
    assert_eq!(auto.timing.as_f64(), Some(1.25));
    assert!(auto.allow_duplicates);
    assert!(!auto.enabled);
    assert!(!auto.only_similar);
    assert_eq!(auto.song_strategy, AutoDjStrategy::Similar);
    assert!(
        !serde_json::to_string(imported.data())
            .unwrap()
            .contains("SYNTHETIC-SECRET")
    );
    assert!(!format!("{:?}", imported.report()).contains("password"));
}
#[test]
fn split_general_is_authoritative_corrupt_split_recovers_independently() {
    let mut split = BTreeMap::new();
    split.insert(
        SettingsCategory::General,
        include_bytes!("../fixtures/p02/imports/general-split.json").as_slice(),
    );
    split.insert(
        SettingsCategory::AutoDJ,
        include_bytes!("../fixtures/p02/imports/autodj-split.json").as_slice(),
    );
    split.insert(SettingsCategory::Lists, b"{".as_slice());
    let imported = preview(
        &CopiedInputs {
            settings: Some(b"invalid old bytes"),
            split_categories: split,
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(imported.data().profile.preferences.language, "de");
    assert!(imported.data().auto_dj.is_some());
    assert!(has(&imported, Code::CorruptCategory));
    assert!(has(&imported, Code::DeferredMigrations));
}
#[test]
fn column_arrays_replace_order_and_empty_arrays_and_corruption_is_per_list() {
    let settings = current_settings(
        &json!({"lists":{"song":{"table":{"columns":[column("name"),column("duration")]}}}}),
    );
    let first = preview(
        &CopiedInputs {
            settings: Some(&settings),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let replacement =
        current_settings(&json!({"lists":{"song":{"table":{"columns":[column("duration")]}}}}));
    let next = preview(
        &CopiedInputs {
            settings: Some(&replacement),
            ..Default::default()
        },
        first.data(),
    )
    .unwrap();
    assert_eq!(next.data().columns[0].columns.len(), 1);
    assert_eq!(next.data().columns[0].columns[0].id, ColumnId::Duration);
    let corrupt = include_bytes!("../fixtures/p02/imports/columns-corrupt.json");
    let input = CopiedInputs {
        split_categories: BTreeMap::from([(SettingsCategory::Lists, corrupt.as_slice())]),
        split_settings_version: Some(34),
        ..Default::default()
    };
    let imported = preview(&input, next.data()).unwrap();
    let song = imported
        .data()
        .columns
        .iter()
        .find(|c| c.list == ListId::Song)
        .unwrap();
    assert_eq!(song.columns, next.data().columns[0].columns);
    assert!(
        imported
            .data()
            .columns
            .iter()
            .find(|c| c.list == ListId::Album)
            .unwrap()
            .columns
            .is_empty()
    );
    assert!(
        imported
            .report()
            .diagnostics
            .iter()
            .any(|d| d.code == Code::DuplicateColumn && d.list == Some(ListId::Song))
    );
    assert!(
        imported
            .report()
            .diagnostics
            .iter()
            .any(|d| d.code == Code::InvalidColumns && d.list == Some(ListId::Artist))
    );
    assert!(
        imported
            .report()
            .diagnostics
            .iter()
            .any(|d| d.code == Code::InvalidColumns && d.list == Some(ListId::Genre))
    );
    let empty = current_settings(&json!({"lists":{"song":{"table":{"columns":[]}}}}));
    assert!(
        preview(
            &CopiedInputs {
                settings: Some(&empty),
                ..Default::default()
            },
            next.data()
        )
        .unwrap()
        .data()
        .columns[0]
            .columns
            .is_empty()
    );
}
#[test]
fn v3_embedded_queue_preserves_occurrences_and_resolves_shuffled_current_timestamp() {
    let input = CopiedInputs {
        player: Some(PLAYER3),
        timestamp: Some(b"12.3459"),
        ..Default::default()
    };
    let imported = preview(&input, &MigrationData::default()).unwrap();
    let q = imported.data().queue.as_ref().unwrap();
    assert_eq!(q.occurrences.len(), 2);
    assert_eq!(q.occurrences[0].song_id, q.occurrences[1].song_id);
    assert_ne!(
        q.occurrences[0].occurrence_id,
        q.occurrences[1].occurrence_id
    );
    assert_eq!(q.occurrences[0].duration_ms, 180000);
    assert_eq!(q.priority, ["second"]);
    assert_eq!(q.shuffled, ["second", "first"]);
    assert_eq!(q.current_occurrence.as_deref(), Some("second"));
    assert_eq!(q.position.as_ref().unwrap().occurrence_id, "second");
    assert_eq!(q.position.as_ref().unwrap().milliseconds, 12345);
    assert!(has(&imported, Code::OrphansRemoved));
    let durable = serde_json::to_string(imported.data()).unwrap();
    for stripped in [
        "SYNTHETIC-SECRET",
        "streamUrl",
        "imageUrl",
        "/copied/private/path",
        "playing",
        "volume",
    ] {
        assert!(!durable.contains(stripped));
    }
}
#[test]
fn v4_split_wins_and_corrupt_existing_split_never_falls_back() {
    let mut split: Value = serde_json::from_slice(QUEUE).unwrap();
    split["songs"]["second"]["name"] = json!("Split winner");
    let split = bytes(&split);
    let imported = preview(
        &CopiedInputs {
            player: Some(PLAYER3),
            separate_queue: Some(&split),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(
        imported.data().queue.as_ref().unwrap().occurrences[1].title,
        "Split winner"
    );
    let rejected = preview(
        &CopiedInputs {
            player: Some(PLAYER3),
            separate_queue: Some(b"corrupt"),
            timestamp: Some(b"4"),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(rejected.data().queue.is_none());
    assert!(has(&rejected, Code::QueueRejected));
    assert!(has(&rejected, Code::TimestampRejected));
    let split = preview(
        &CopiedInputs {
            player: Some(PLAYER4),
            separate_queue: Some(QUEUE),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(
        split
            .data()
            .queue
            .as_ref()
            .unwrap()
            .current_occurrence
            .as_deref(),
        Some("second")
    );
    assert!(split.data().queue.as_ref().unwrap().shuffled.is_empty());
}
#[test]
fn missing_map_duplicate_occurrence_priority_and_invalid_shuffle_are_explicit() {
    let mut player: Value = serde_json::from_slice(PLAYER3).unwrap();
    player["state"]["queue"]["default"] = json!(["first", "missing", "second", "first"]);
    player["state"]["queue"]["priority"] = json!(["missing", "second", "second"]);
    player["state"]["queue"]["shuffled"] = json!([0, 0, 1, 2]);
    let player = bytes(&player);
    let imported = preview(
        &CopiedInputs {
            player: Some(&player),
            timestamp: Some(b"5"),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let q = imported.data().queue.as_ref().unwrap();
    assert_eq!(q.occurrences.len(), 2);
    assert_eq!(q.priority, ["second"]);
    assert!(q.shuffled.is_empty());
    assert!(q.current_occurrence.is_none());
    assert!(q.position.is_none());
    for code in [
        Code::MissingSong,
        Code::DuplicateOccurrence,
        Code::PriorityRepaired,
        Code::ShuffleRejected,
        Code::CurrentRejected,
        Code::TimestampRejected,
    ] {
        assert!(has(&imported, code));
    }
}
#[test]
fn timestamps_are_checked_and_require_a_new_matching_current_occurrence() {
    for value in ["-1", "1e100", "null", "\"token\""] {
        let imported = preview(
            &CopiedInputs {
                player: Some(PLAYER4),
                separate_queue: Some(QUEUE),
                timestamp: Some(value.as_bytes()),
                ..Default::default()
            },
            &MigrationData::default(),
        )
        .unwrap();
        assert!(has(&imported, Code::TimestampRejected));
        assert!(imported.data().queue.as_ref().unwrap().position.is_none());
    }
    let input = CopiedInputs {
        player: Some(PLAYER4),
        separate_queue: Some(QUEUE),
        timestamp: Some(br#"{"version":1,"state":{"timestamp":0.5}}"#),
        ..Default::default()
    };
    let imported = preview(&input, &MigrationData::default()).unwrap();
    assert_eq!(
        imported
            .data()
            .queue
            .as_ref()
            .unwrap()
            .position
            .as_ref()
            .unwrap()
            .milliseconds,
        500
    );
    let unmatched = preview(
        &CopiedInputs {
            timestamp: Some(b"8"),
            ..Default::default()
        },
        imported.data(),
    )
    .unwrap();
    assert!(has(&unmatched, Code::TimestampRejected));
    assert_eq!(unmatched.data(), imported.data());
    let mut player: Value = serde_json::from_slice(PLAYER4).unwrap();
    player["state"]["player"]["index"] = json!(99);
    let player = bytes(&player);
    let missing = preview(
        &CopiedInputs {
            player: Some(&player),
            separate_queue: Some(QUEUE),
            timestamp: Some(b"0"),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(has(&missing, Code::CurrentRejected));
    assert!(has(&missing, Code::TimestampRejected));
}
#[test]
fn copied_auth_projects_only_non_secret_profiles_and_clears_invalid_selection() {
    let imported = preview(
        &CopiedInputs {
            auth: Some(AUTH),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(imported.data().profile.profiles.len(), 1);
    assert_eq!(
        imported.data().profile.current_server.as_deref(),
        Some("server-a")
    );
    assert_eq!(
        imported.data().profile.profiles[0].endpoint,
        "http://192.168.1.2/music"
    );
    assert!(has(&imported, Code::CredentialsStripped));
    let durable = serde_json::to_string(imported.data()).unwrap();
    for stripped in [
        "SYNTHETIC",
        "private-user",
        "private-device",
        "credential",
        "savePassword",
    ] {
        assert!(!durable.contains(stripped));
    }
    let mut auth: Value = serde_json::from_slice(AUTH).unwrap();
    auth["state"]["serverList"]["server-a"]["url"] = json!("http://user:secret@192.168.1.2");
    let auth = bytes(&auth);
    let invalid = preview(
        &CopiedInputs {
            auth: Some(&auth),
            ..Default::default()
        },
        imported.data(),
    )
    .unwrap();
    assert!(invalid.data().profile.profiles.is_empty());
    assert!(invalid.data().profile.current_server.is_none());
    assert!(has(&invalid, Code::InvalidProfile));
    assert!(has(&invalid, Code::OrphanSelection));
}
#[test]
fn unsupported_versions_and_malformed_envelopes_fail_without_partial_success() {
    for settings in [
        br#"{"version":999,"general":{}}"#.as_slice(),
        br#"{"version":13,"general":{}}"#,
        br#"{"version":28,"general":{}}"#,
    ] {
        assert_eq!(
            preview(
                &CopiedInputs {
                    settings: Some(settings),
                    ..Default::default()
                },
                &MigrationData::default()
            ),
            Err(ImportError::UnsupportedVersion)
        );
    }
    assert_eq!(
        preview(
            &CopiedInputs {
                auth: Some(br#"{"version":1,"state":{}}"#),
                ..Default::default()
            },
            &MigrationData::default()
        ),
        Err(ImportError::UnsupportedVersion)
    );
    for version in [2, 5] {
        let player = bytes(&json!({"version":version,"state":{}}));
        assert_eq!(
            preview(
                &CopiedInputs {
                    player: Some(&player),
                    ..Default::default()
                },
                &MigrationData::default()
            ),
            Err(ImportError::UnsupportedVersion)
        );
    }
    assert_eq!(
        preview(
            &CopiedInputs {
                timestamp: Some(br#"{"version":2,"state":{"timestamp":1}}"#),
                ..Default::default()
            },
            &MigrationData::default()
        ),
        Err(ImportError::UnsupportedVersion)
    );
    for settings in [
        b"{".as_slice(),
        br#"{"general":{}}"#,
        br#"{"version":34,"state":null}"#,
    ] {
        assert_eq!(
            preview(
                &CopiedInputs {
                    settings: Some(settings),
                    ..Default::default()
                },
                &MigrationData::default()
            ),
            Err(ImportError::InvalidDocument)
        );
    }
}
#[test]
fn preview_is_deterministic_idempotent_and_redacted_reports_are_capped() {
    let input = CopiedInputs {
        settings: Some(OLD),
        player: Some(PLAYER3),
        auth: Some(AUTH),
        timestamp: Some(b"1"),
        ..Default::default()
    };
    let base = MigrationData::default();
    let first = preview(&input, &base).unwrap();
    assert_eq!(first, preview(&input, &base).unwrap());
    assert_eq!(first.data(), preview(&input, first.data()).unwrap().data());
    assert_eq!(base, MigrationData::default());
    let mut state = serde_json::Map::new();
    for i in 0..100 {
        state.insert(format!("SECRET-RAW-KEY-{i}"), json!("SECRET-RAW-VALUE"));
    }
    let settings = current_settings(&Value::Object(state));
    let capped = preview(
        &CopiedInputs {
            settings: Some(&settings),
            ..Default::default()
        },
        &base,
    )
    .unwrap();
    assert_eq!(capped.report().diagnostics.len(), MAX_DIAGNOSTICS);
    assert!(capped.report().omitted_diagnostics > 0);
    assert!(!format!("{:?}", capped.report()).contains("SECRET"));
    assert!(
        !serde_json::to_string(capped.data())
            .unwrap()
            .contains("SECRET")
    );
}
#[test]
fn input_output_queue_and_durable_invariants_are_bounded() {
    let large = vec![b' '; MAX_INPUT_BYTES + 1];
    assert_eq!(
        preview(
            &CopiedInputs {
                settings: Some(&large),
                ..Default::default()
            },
            &MigrationData::default()
        ),
        Err(ImportError::InputTooLarge)
    );
    // Total across independent copies, not just each individual document.
    let half = vec![b' '; MAX_INPUT_BYTES / 2 + 1];
    assert_eq!(
        preview(
            &CopiedInputs {
                settings: Some(&half),
                auth: Some(&half),
                ..Default::default()
            },
            &MigrationData::default()
        ),
        Err(ImportError::InputTooLarge)
    );
    let mut player: Value = serde_json::from_slice(PLAYER3).unwrap();
    let q = &mut player["state"]["queue"];
    q["default"] = json!(
        (0..MAX_QUEUE + 1)
            .map(|n| format!("o{n}"))
            .collect::<Vec<_>>()
    );
    let large_queue = bytes(&player);
    let rejected = preview(
        &CopiedInputs {
            player: Some(&large_queue),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(has(&rejected, Code::QueueRejected));
    assert!(rejected.data().queue.is_none());
    let mut songs = serde_json::Map::new();
    let mut ids = vec![];
    for n in 0..300 {
        let id = format!("o{n}");
        ids.push(id.clone());
        songs.insert(id.clone(),json!({"id":"song","_serverId":"s","_uniqueId":id,"name":"x".repeat(1024),"duration":0}));
    }
    let queue = bytes(&json!({"default":ids,"priority":[],"shuffled":[],"songs":songs}));
    assert_eq!(
        preview(
            &CopiedInputs {
                player: Some(PLAYER4),
                separate_queue: Some(&queue),
                ..Default::default()
            },
            &MigrationData::default()
        ),
        Err(ImportError::OutputTooLarge)
    );
    let valid = preview(
        &CopiedInputs {
            player: Some(PLAYER3),
            timestamp: Some(b"1"),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let mut data = valid.data().clone();
    data.queue
        .as_mut()
        .unwrap()
        .position
        .as_mut()
        .unwrap()
        .occurrence_id = "first".into();
    assert_eq!(validate_data(&data), Err(ImportError::InvalidData));
    let mut data = valid.data().clone();
    data.queue.as_mut().unwrap().priority.push("missing".into());
    assert_eq!(validate_data(&data), Err(ImportError::InvalidData));
    let mut data = valid.data().clone();
    data.queue.as_mut().unwrap().shuffled = vec!["first".into(), "first".into()];
    assert_eq!(validate_data(&data), Err(ImportError::InvalidData));
}

#[test]
fn typed_domain_bridges_keep_song_occurrence_server_and_units_distinct() {
    use feishin_domain::{Milliseconds, PlaybackPosition, QueueOccurrenceId, ServerId, SongId};
    let imported = preview(
        &CopiedInputs {
            player: Some(PLAYER3),
            timestamp: Some(b"2"),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let q = imported.data().queue.as_ref().unwrap();
    assert_eq!(
        q.occurrences[0].occurrence_id(),
        QueueOccurrenceId::new("first")
    );
    assert_eq!(q.occurrences[0].song().id, SongId::new("same-song"));
    assert_eq!(q.occurrences[0].song().server, ServerId::new("server-a"));
    assert_eq!(q.occurrences[0].duration(), Milliseconds(180000));
    let position = q.position.as_ref().unwrap();
    assert_eq!(position.occurrence_id(), QueueOccurrenceId::new("second"));
    assert_eq!(position.position(), PlaybackPosition(Milliseconds(2000)));
}
#[test]
fn mixed_legacy_copies_keep_existing_split_autodj_and_report_overwritten_split_lists() {
    let mut combined: Value = serde_json::from_slice(OLD).unwrap();
    combined["state"].as_object_mut().unwrap().remove("autoDJ");
    let combined = bytes(&combined);
    let imported = preview(
        &CopiedInputs {
            settings: Some(&combined),
            split_categories: BTreeMap::from([
                (
                    SettingsCategory::AutoDJ,
                    include_bytes!("../fixtures/p02/imports/autodj-split.json").as_slice(),
                ),
                (
                    SettingsCategory::Lists,
                    b"corrupt stale category".as_slice(),
                ),
            ]),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(
        imported.data().auto_dj.as_ref().unwrap().mode,
        AutoDjMode::Albums
    );
    assert_eq!(imported.data().columns.len(), 2);
    assert!(has(&imported, Code::IgnoredSplitCopy));
}
#[test]
fn characterized_source_v3_fixture_cleans_map_and_retains_repeated_song_without_unique_field() {
    let traces: Value =
        serde_json::from_slice(include_bytes!("../fixtures/p00/state-traces.json")).unwrap();
    fn find(value: &Value) -> Option<&Value> {
        if value.get("id").and_then(Value::as_str) == Some("embedded-queue-v3-migration") {
            return value.get("observed");
        }
        match value {
            Value::Array(v) => v.iter().find_map(find),
            Value::Object(v) => v.values().find_map(find),
            _ => None,
        }
    }
    let observed = bytes(find(&traces).unwrap());
    let imported = preview(
        &CopiedInputs {
            player: Some(&observed),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let q = imported.data().queue.as_ref().unwrap();
    assert_eq!(q.occurrences.len(), 2);
    assert_eq!(q.occurrences[0].song_id, q.occurrences[1].song_id);
    assert_eq!(q.occurrences[0].duration_ms, 180000);
    assert!(q.current_occurrence.is_none()); // The fixture supplies no index or shuffle mode.
    assert!(has(&imported, Code::CurrentRejected));
}

#[test]
fn album_shuffle_and_empty_track_permutation_use_default_current_order() {
    for (mode, permutation) in [
        ("album", json!([1, 0])),
        ("album", json!([])),
        ("track", json!([])),
        ("none", json!([99])),
    ] {
        let mut player: Value = serde_json::from_slice(PLAYER3).unwrap();
        player["state"]["player"]["shuffle"] = json!(mode);
        player["state"]["queue"]["shuffled"] = permutation;
        let player = bytes(&player);
        let imported = preview(
            &CopiedInputs {
                player: Some(&player),
                timestamp: Some(b"3"),
                ..Default::default()
            },
            &MigrationData::default(),
        )
        .unwrap();
        let queue = imported.data().queue.as_ref().unwrap();
        assert!(queue.shuffled.is_empty());
        assert_eq!(queue.current_occurrence.as_deref(), Some("first"));
        assert_eq!(queue.position.as_ref().unwrap().occurrence_id, "first");
        assert_eq!(queue.position.as_ref().unwrap().milliseconds, 3000);
        assert!(!has(&imported, Code::ShuffleRejected));
    }
}

#[test]
fn recovered_default_order_never_guesses_current_or_attaches_timestamp() {
    let cases = [
        ("missing prefix", json!(["missing", "first", "second"]), 1),
        ("missing middle", json!(["first", "missing", "second"]), 1),
        ("missing suffix", json!(["first", "second", "missing"]), 0),
        ("invalid record", json!(["invalid", "first", "second"]), 1),
        (
            "duplicate occurrence",
            json!(["first", "first", "second"]),
            1,
        ),
    ];
    for mode in ["none", "album", "track"] {
        for (case, default, index) in &cases {
            let mut player: Value = serde_json::from_slice(PLAYER3).unwrap();
            player["state"]["player"]["shuffle"] = json!(mode);
            player["state"]["player"]["index"] = json!(index);
            player["state"]["queue"]["default"] = default.clone();
            player["state"]["queue"]["shuffled"] = json!([]);
            player["state"]["queue"]["songs"]["invalid"] = json!({
                "id":"song-invalid", "_serverId":"server-a", "name":"Invalid duration", "duration":"bad"
            });
            let player = bytes(&player);
            let imported = preview(
                &CopiedInputs {
                    player: Some(&player),
                    timestamp: Some(b"12.5"),
                    ..Default::default()
                },
                &MigrationData::default(),
            )
            .unwrap();
            let queue = imported.data().queue.as_ref().unwrap();
            assert_eq!(
                queue
                    .occurrences
                    .iter()
                    .map(|o| o.occurrence_id.as_str())
                    .collect::<Vec<_>>(),
                ["first", "second"],
                "{mode}: {case}"
            );
            assert_eq!(queue.priority, ["second"], "{mode}: {case}");
            assert!(queue.current_occurrence.is_none(), "{mode}: {case}");
            assert!(queue.position.is_none(), "{mode}: {case}");
            assert!(has(&imported, Code::CurrentRejected), "{mode}: {case}");
            assert!(has(&imported, Code::TimestampRejected), "{mode}: {case}");
            assert!(!has(&imported, Code::ShuffleRejected), "{mode}: {case}");
            assert!(
                has(
                    &imported,
                    if *case == "duplicate occurrence" {
                        Code::DuplicateOccurrence
                    } else {
                        Code::MissingSong
                    }
                ),
                "{mode}: {case}"
            );
        }
    }
}
