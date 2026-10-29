use feishin_storage::{imports::*, settings_migration::*};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn source() -> Value {
    serde_json::from_slice(include_bytes!(
        "../fixtures/p02/settings-migrations/source-replay.json"
    ))
    .unwrap()
}
fn column(id: &str) -> Value {
    json!({"id":id,"align":"end","autoSize":false,"isEnabled":true,"pinned":"right","width":17})
}
fn input(version: u64, state: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"version":version,"state":state})).unwrap()
}
fn has(p: &ImportPreview, code: Code) -> bool {
    p.report().diagnostics.iter().any(|d| d.code == code)
}

#[derive(Deserialize)]
struct ColumnCase {
    version: u64,
    list: ListId,
    view: ColumnView,
    shape: String,
}
#[test]
fn retained_columns_match_273_pinned_original_migration_outputs() {
    let source = source();
    assert_eq!(
        source["baseline"],
        "7d6ad318c79802e226996cfc39acba6b05bb22e8"
    );
    assert_eq!(
        source["methodSha256"],
        "4113f0fd417709d00fbd141e4a4595845dbde7eb2be9c2560b86796958b293ac"
    );
    let cases: Vec<ColumnCase> = serde_json::from_value(source["cases"].clone()).unwrap();
    assert_eq!(cases.len(), 273);
    let shapes: BTreeMap<String, Vec<Column>> =
        serde_json::from_value(source["shapes"].clone()).unwrap();
    for case in cases {
        let mut columns: Vec<Column> = serde_json::from_value(source["input"].clone()).unwrap();
        migrate_columns(&mut columns, case.list, case.view, case.version).unwrap();
        assert_eq!(
            columns, shapes[&case.shape],
            "version {} {:?} {:?}",
            case.version, case.list, case.view
        );
        let once = columns.clone();
        assert!(!migrate_columns(&mut columns, case.list, case.view, case.version).unwrap());
        assert_eq!(columns, once);
    }
}
#[test]
fn autodj_matches_110_source_cases_and_actual_source_default_merge() {
    let source = source();
    let cases = source["autoCases"].as_array().unwrap();
    assert_eq!(cases.len(), 110);
    for case in cases {
        let version = case["version"].as_u64().unwrap();
        let expected = serde_json::from_value::<AutoDj>(case["output"].clone());
        let actual = migrate_auto_dj(&case["input"], version);
        match expected {
            Ok(expected) => assert_eq!(actual.unwrap(), expected),
            Err(_) => assert_eq!(actual, Err(ImportError::InvalidData)),
        }
    }
    let sparse = &source["sparseAutoDj"];
    let expected: AutoDj = serde_json::from_value(sparse["merged"].clone()).unwrap();
    assert_eq!(migrate_auto_dj(&sparse["input"], 27).unwrap(), expected);
}
#[test]
fn duplicate_insertion_difference_and_source_key_spelling_are_explicit() {
    let source = source();
    let observed = &source["observations"];
    let original: Vec<Column> = serde_json::from_value(observed["duplicated"].clone()).unwrap();
    assert_eq!(
        original
            .iter()
            .filter(|c| c.id == ColumnId::BitDepth)
            .count(),
        2
    );
    let mut expected = original.clone();
    let mut seen = BTreeSet::new();
    expected.retain(|c| seen.insert(c.id));
    let mut actual: Vec<Column> =
        serde_json::from_value(observed["duplicateInput"].clone()).unwrap();
    migrate_columns(&mut actual, ListId::Song, ColumnView::Table, 14).unwrap();
    assert_eq!(actual, expected);
    for key_case in observed["keyCases"].as_array().unwrap() {
        let name = key_case["list"].as_str().unwrap();
        let columns: Vec<Column> = serde_json::from_value(key_case["output"].clone()).unwrap();
        let count = columns
            .iter()
            .filter(|c| c.id == ColumnId::BitDepth)
            .count();
        assert_eq!(
            count,
            usize::from(matches!(name, "fullscreen" | "sidequeue"))
        );
    }
    let raw = input(
        14,
        json!({"lists":{"fullscreen":{"table":{"columns":[]}},"sidequeue":{"table":{"columns":[]}}}}),
    );
    let result = preview(
        &CopiedInputs {
            settings: Some(&raw),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(result.data().columns.is_empty());
    assert!(has(&result, Code::UnsupportedFields));
}
#[test]
fn every_supported_version_works_through_combined_and_split_projection() {
    let columns = json!([
        column("titleCombined"),
        column("artists"),
        column("releaseYear"),
        column("releaseDate")
    ]);
    for version in 14..=34 {
        let strategy = if version < 28 {
            "similar_forward"
        } else {
            "library_random"
        };
        let auto = json!({"albumStrategy":strategy,"allowDuplicates":true,"enabled":false,"itemCount":7.5,"mode":"albums","onlySimilar":false,"songStrategy":strategy,"timing":1.25});
        let lists = json!({"song":{"table":{"columns":columns}}});
        let raw = input(version, json!({"lists":lists,"autoDJ":auto}));
        let lists_bytes = serde_json::to_vec(&lists).unwrap();
        let auto_bytes = serde_json::to_vec(&auto).unwrap();
        let combined = preview(
            &CopiedInputs {
                settings: Some(&raw),
                ..Default::default()
            },
            &MigrationData::default(),
        )
        .unwrap();
        let split = preview(
            &CopiedInputs {
                split_categories: BTreeMap::from([
                    (SettingsCategory::Lists, lists_bytes.as_slice()),
                    (SettingsCategory::AutoDJ, auto_bytes.as_slice()),
                ]),
                split_settings_version: Some(version),
                ..Default::default()
            },
            &MigrationData::default(),
        )
        .unwrap();
        assert_eq!(combined.data(), split.data());
        assert_eq!(
            combined.data().auto_dj.as_ref().unwrap().mode,
            AutoDjMode::Albums
        );
        let once = preview(
            &CopiedInputs {
                settings: Some(&raw),
                ..Default::default()
            },
            combined.data(),
        )
        .unwrap();
        assert_eq!(once.data(), combined.data());
        assert_eq!(
            has(&combined, Code::HistoricalTransformApplied),
            version < 29
        );
        assert_eq!(has(&combined, Code::DeferredMigrations), version < 34);
    }
}
#[test]
fn mixed_copy_transforms_use_the_version_of_the_selected_category() {
    let strict_auto = json!({"albumStrategy":"library_random","allowDuplicates":true,"enabled":false,"itemCount":9,"mode":"albums","onlySimilar":false,"songStrategy":"library_random","timing":2});
    let combined = input(
        34,
        json!({"autoDJ":strict_auto,"lists":{"song":{"table":{"columns":[column("duration")]}}}}),
    );
    let old_auto = br#"{"mode":"old","songStrategy":"default"}"#;
    let old_lists = serde_json::to_vec(&json!({"song":{"table":{"columns":[]}}})).unwrap();
    let imported = preview(
        &CopiedInputs {
            settings: Some(&combined),
            split_categories: BTreeMap::from([
                (SettingsCategory::AutoDJ, old_auto.as_slice()),
                (SettingsCategory::Lists, old_lists.as_slice()),
            ]),
            split_settings_version: Some(27),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(
        imported.data().auto_dj.as_ref().unwrap().song_strategy,
        AutoDjStrategy::Similar
    );
    assert_eq!(
        imported.data().auto_dj.as_ref().unwrap().mode,
        AutoDjMode::Songs
    );
    assert_eq!(imported.data().columns[0].columns.len(), 1);
    assert_eq!(imported.data().columns[0].columns[0].id, ColumnId::Duration);
    let old = input(
        27,
        json!({"autoDJ":{"mode":"old","songStrategy":"default"}}),
    );
    let strict_bytes = serde_json::to_vec(&strict_auto).unwrap();
    let imported = preview(
        &CopiedInputs {
            settings: Some(&old),
            split_categories: BTreeMap::from([(SettingsCategory::AutoDJ, strict_bytes.as_slice())]),
            split_settings_version: Some(34),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert_eq!(
        imported.data().auto_dj.as_ref().unwrap().song_strategy,
        AutoDjStrategy::LibraryRandom
    );
    let invalid_current = preview(
        &CopiedInputs {
            split_categories: BTreeMap::from([(SettingsCategory::AutoDJ, old_auto.as_slice())]),
            split_settings_version: Some(34),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    assert!(invalid_current.data().auto_dj.is_none());
    assert!(has(&invalid_current, Code::CorruptCategory));
}
#[test]
fn absent_categories_and_rejected_arrays_preserve_the_target_baseline() {
    let baseline = input(
        34,
        json!({"autoDJ":{"albumStrategy":"library_random","allowDuplicates":true,"enabled":true,"itemCount":9,"mode":"albums","onlySimilar":false,"songStrategy":"library_random","timing":2},"lists":{"song":{"table":{"columns":[column("duration")]}}}}),
    );
    let base = preview(
        &CopiedInputs {
            settings: Some(&baseline),
            ..Default::default()
        },
        &MigrationData::default(),
    )
    .unwrap();
    let partial = input(
        20,
        json!({"general":{"language":"uk"},"lists":{"song":{"table":{"columns":[column("name"),column("name")]}}}}),
    );
    let result = preview(
        &CopiedInputs {
            settings: Some(&partial),
            ..Default::default()
        },
        base.data(),
    )
    .unwrap();
    assert_eq!(result.data().auto_dj, base.data().auto_dj);
    assert_eq!(result.data().columns, base.data().columns);
    assert!(has(&result, Code::LegacyDefaultsDeferred));
    assert!(has(&result, Code::DuplicateColumn));
    let legacy_empty = input(14, json!({"lists":{"song":{"table":{"columns":[]}}}}));
    let result = preview(
        &CopiedInputs {
            settings: Some(&legacy_empty),
            ..Default::default()
        },
        base.data(),
    )
    .unwrap();
    assert_eq!(result.data().columns[0].columns.len(), 7);
    assert!(
        result.data().columns[0]
            .columns
            .iter()
            .all(|c| !c.is_enabled)
    );
    let current_empty = input(34, json!({"lists":{"song":{"table":{"columns":[]}}}}));
    let result = preview(
        &CopiedInputs {
            settings: Some(&current_empty),
            ..Default::default()
        },
        base.data(),
    )
    .unwrap();
    assert!(result.data().columns[0].columns.is_empty());
}
