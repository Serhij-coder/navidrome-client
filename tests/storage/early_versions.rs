use feishin_storage::{BackendKind, Preferences, Profile, imports::*, settings_migration::*};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn fixture() -> Value {
    serde_json::from_slice(include_bytes!(
        "../fixtures/p02/settings-migrations/early-source-replay.json"
    ))
    .unwrap()
}
fn bytes(version: u64, state: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"version":version,"state":state})).unwrap()
}
fn has(p: &ImportPreview, code: Code) -> bool {
    p.report().diagnostics.iter().any(|d| d.code == code)
}
fn base() -> MigrationData {
    let raw = bytes(
        34,
        json!({"lists":{"song":{"table":{"columns":[{"id":"duration","align":"end","autoSize":false,"isEnabled":true,"pinned":null,"width":73}]}}},"autoDJ":{"mode":"albums","albumStrategy":"library_random","songStrategy":"library_random","allowDuplicates":true,"onlySimilar":true,"enabled":true,"itemCount":9,"timing":2}}),
    );
    let mut data = preview(
        &CopiedInputs {
            settings: Some(&raw),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap()
    .data()
    .clone();
    data.profile.preferences = Preferences {
        language: "de".into(),
        follow_system_theme: false,
    };
    data.profile.profiles = vec![Profile {
        id: "server".into(),
        name: "Fixture".into(),
        endpoint: "http://192.0.2.4:4533".into(),
        backend: BackendKind::Navidrome,
    }];
    data.profile.current_server = Some("server".into());
    data.queue = Some(QueueSnapshot {
        occurrences: vec![QueueOccurrence {
            occurrence_id: "occurrence".into(),
            song_id: "song".into(),
            server_id: "server".into(),
            title: "Fixture".into(),
            duration_ms: 10000,
        }],
        priority: vec![],
        shuffled: vec![],
        current_occurrence: Some("occurrence".into()),
        position: Some(ResumePosition {
            occurrence_id: "occurrence".into(),
            milliseconds: 2500,
        }),
    });
    validate_data(&data).unwrap();
    data
}
fn assert_unrelated(actual: &MigrationData, previous: &MigrationData) {
    assert_eq!(actual.profile.profiles, previous.profile.profiles);
    assert_eq!(
        actual.profile.current_server,
        previous.profile.current_server
    );
    assert_eq!(actual.queue, previous.queue);
}
#[test]
fn all_56_early_source_cases_match_retained_rules_in_combined_and_split_copies() {
    let source = fixture();
    assert_eq!(
        source["methodSha256"],
        "4113f0fd417709d00fbd141e4a4595845dbde7eb2be9c2560b86796958b293ac"
    );
    let cases = source["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 56);
    let baseline = base();
    for case in cases {
        let version = case["version"].as_u64().unwrap();
        let variant = case["variant"].as_str().unwrap();
        let raw = bytes(version, case["input"].clone());
        let unchanged = raw.clone();
        let result = preview(
            &CopiedInputs {
                settings: Some(&raw),
                ..Default::default()
            },
            &baseline,
        )
        .unwrap();
        let list_bytes = serde_json::to_vec(&case["input"]["lists"]).unwrap();
        let general_bytes = serde_json::to_vec(&case["input"]["general"]).unwrap();
        let auto_bytes = serde_json::to_vec(&case["input"]["autoDJ"]).unwrap();
        let mut split = BTreeMap::from([
            (SettingsCategory::General, general_bytes.as_slice()),
            (SettingsCategory::Lists, list_bytes.as_slice()),
        ]);
        if variant != "missing" {
            split.insert(SettingsCategory::AutoDJ, auto_bytes.as_slice());
        }
        let split_result = preview(
            &CopiedInputs {
                split_categories: split,
                split_settings_version: Some(version),
                ..Default::default()
            },
            &baseline,
        )
        .unwrap();
        assert_eq!(
            result.data(),
            split_result.data(),
            "split v{version} {variant}"
        );
        assert_unrelated(result.data(), &baseline);
        assert_eq!(raw, unchanged);
        assert!(result.report().partial);
        assert!(has(&result, Code::DeferredMigrations));
        assert_eq!(
            result.data().profile.preferences.language,
            case["general"]["language"].as_str().unwrap()
        );
        assert_eq!(
            result.data().profile.preferences.follow_system_theme,
            case["general"]["followSystemTheme"].as_bool().unwrap()
        );
        if case["returnedEmpty"] == true {
            assert!(matches!(version, 10 | 11));
            assert!(result.data().columns.is_empty());
            let expected: AutoDj = serde_json::from_value(case["autoDJ"].clone()).unwrap();
            assert_eq!(result.data().auto_dj, Some(expected));
            assert!(has(&result, Code::SettingsReset));
            assert!(!has(&result, Code::LegacyDefaultsDeferred));
        } else {
            let expected: Vec<Column> = serde_json::from_value(case["columns"].clone()).unwrap();
            assert_eq!(
                result.data().columns[0].columns,
                expected,
                "columns v{version}"
            );
            if variant == "missing" {
                assert_eq!(result.data().auto_dj, baseline.auto_dj);
                assert!(has(&result, Code::LegacyDefaultsDeferred));
            } else if let Ok(expected) = serde_json::from_value::<AutoDj>(case["autoDJ"].clone()) {
                assert_eq!(
                    result.data().auto_dj,
                    Some(expected),
                    "AutoDJ v{version} {variant}"
                );
            } else {
                assert!(version <= 9);
                assert_eq!(result.data().auto_dj, baseline.auto_dj);
                assert!(has(&result, Code::CorruptCategory));
            }
            assert_eq!(has(&result, Code::EarlySettingsReturn), version <= 9);
        }
    }
    let v8 = cases
        .iter()
        .find(|c| c["version"] == 8 && c["variant"] == "valid")
        .unwrap();
    assert_eq!(v8["sidebarIds"], json!(["Artists-all"]));
    assert_eq!(v8["windowBarStyle"], "linux");
}
#[test]
fn full_reset_dominates_supplemental_copies_and_authoritative_split_ignores_combined() {
    let baseline = base();
    let reset = bytes(
        10,
        json!({"general":{"sidebarItems":[]},"autoDJ":{"credential":"SYNTHETIC-CANARY"}}),
    );
    let stronger_auto = serde_json::to_vec(baseline.auto_dj.as_ref().unwrap()).unwrap();
    let result = preview(
        &CopiedInputs {
            settings: Some(&reset),
            split_categories: BTreeMap::from([(
                SettingsCategory::AutoDJ,
                stronger_auto.as_slice(),
            )]),
            split_settings_version: Some(34),
            ..Default::default()
        },
        &baseline,
    )
    .unwrap();
    assert_eq!(result.data().auto_dj, Some(default_auto_dj()));
    assert!(result.data().columns.is_empty());
    assert!(has(&result, Code::SettingsReset));
    assert!(has(&result, Code::IgnoredSplitCopy));
    assert_unrelated(result.data(), &baseline);
    assert!(
        !serde_json::to_string(result.data())
            .unwrap()
            .contains("SYNTHETIC-CANARY")
    );
    let general = b"{}";
    let malformed = b"not-json";
    let result = preview(
        &CopiedInputs {
            settings: Some(malformed),
            split_categories: BTreeMap::from([
                (SettingsCategory::General, general.as_slice()),
                (SettingsCategory::Lists, malformed.as_slice()),
            ]),
            split_settings_version: Some(11),
            ..Default::default()
        },
        &baseline,
    )
    .unwrap();
    assert_eq!(result.data().profile.preferences, Preferences::default());
    assert!(result.data().columns.is_empty());
    assert_eq!(result.data().auto_dj, Some(default_auto_dj()));
    assert_unrelated(result.data(), &baseline);
}
#[test]
fn supplemental_reset_changes_only_its_selected_category() {
    let baseline = base();
    let current = bytes(
        34,
        json!({"general":{"language":"it","followSystemTheme":true}}),
    );
    for category in [SettingsCategory::AutoDJ, SettingsCategory::Lists] {
        let result = preview(
            &CopiedInputs {
                settings: Some(&current),
                split_categories: BTreeMap::from([(category, b"null".as_slice())]),
                split_settings_version: Some(11),
                ..Default::default()
            },
            &baseline,
        )
        .unwrap();
        assert_eq!(result.data().profile.preferences.language, "it");
        assert!(result.data().profile.preferences.follow_system_theme);
        if category == SettingsCategory::AutoDJ {
            assert_eq!(result.data().auto_dj, Some(default_auto_dj()));
            assert_eq!(result.data().columns, baseline.columns);
        } else {
            assert!(result.data().columns.is_empty());
            assert_eq!(result.data().auto_dj, baseline.auto_dj);
        }
        assert!(has(&result, Code::SettingsReset));
        assert_unrelated(result.data(), &baseline);
    }
    let current = bytes(
        34,
        json!({"general":{"language":"it"},"lists":{"song":{"table":{"columns":[]}}}}),
    );
    let result = preview(
        &CopiedInputs {
            settings: Some(&current),
            split_categories: BTreeMap::from([(SettingsCategory::Lists, b"null".as_slice())]),
            split_settings_version: Some(10),
            ..Default::default()
        },
        &baseline,
    )
    .unwrap();
    assert!(has(&result, Code::IgnoredSplitCopy));
    assert!(!has(&result, Code::SettingsReset));
    assert_eq!(result.data().columns.len(), 1);
    assert!(result.data().columns[0].columns.is_empty());
}
#[test]
fn malformed_reset_envelopes_fail_closed_instead_of_resetting() {
    let source = fixture();
    let baseline = base();
    for case in source["invalidResetCases"].as_array().unwrap() {
        let raw = bytes(case["version"].as_u64().unwrap(), case["input"].clone());
        assert_eq!(
            preview(
                &CopiedInputs {
                    settings: Some(&raw),
                    ..Default::default()
                },
                &baseline
            ),
            Err(ImportError::InvalidDocument)
        );
    }
    for version in [10, 11] {
        let raw = bytes(version, json!({"general":{"sidebarItems":[]}}));
        assert_eq!(
            preview(
                &CopiedInputs {
                    settings: Some(&raw),
                    split_categories: BTreeMap::from([(
                        SettingsCategory::AutoDJ,
                        b"{}".as_slice()
                    )]),
                    split_settings_version: Some(35),
                    ..Default::default()
                },
                &baseline
            ),
            Err(ImportError::UnsupportedVersion)
        );
    }
    let split = CopiedInputs {
        split_categories: BTreeMap::from([(SettingsCategory::General, b"{broken".as_slice())]),
        split_settings_version: Some(10),
        ..Default::default()
    };
    assert_eq!(
        preview(&split, &baseline),
        Err(ImportError::InvalidDocument)
    );
}
#[test]
fn helper_reset_cannot_be_misused_as_an_empty_array_and_early_aliases_do_not_normalize() {
    let source = fixture();
    let original: Vec<Column> =
        serde_json::from_value(source["cases"][0]["columns"].clone()).unwrap();
    for version in 0..=34 {
        let mut columns = original.clone();
        let path = settings_path(version).unwrap();
        let result = migrate_columns(&mut columns, ListId::Song, ColumnView::Table, version);
        if path == SettingsPath::Reset {
            assert_eq!(result, Err(ImportError::SettingsReset));
            assert_eq!(columns, original);
        } else {
            result.unwrap();
        }
    }
    let sparse = json!({"enabled":true});
    for version in 0..=9 {
        assert!(migrate_auto_dj(&json!({"mode":"old","songStrategy":"default"}), version).is_err());
        assert!(migrate_auto_dj(&sparse, version).unwrap().enabled);
    }
}
