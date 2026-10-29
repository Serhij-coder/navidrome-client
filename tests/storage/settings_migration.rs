use feishin_storage::imports::{
    Alignment, Column, ColumnId as Id, ColumnView as View, ListId as List, Pin,
};
use feishin_storage::settings_migration::{
    SettingsPath, default_auto_dj, migrate_auto_dj, migrate_columns, settings_path,
};
use serde_json::{Number, json};

fn col(id: Id) -> Column {
    Column {
        id,
        align: Alignment::End,
        auto_size: None,
        is_enabled: true,
        pinned: Some(Pin::Right),
        width: Number::from(17),
    }
}
fn ids(columns: &[Column]) -> Vec<Id> {
    columns.iter().map(|c| c.id).collect()
}

#[test]
fn thresholds_order_properties_and_idempotence() {
    for version in [14, 15, 20, 21, 22, 26, 27, 28, 29, 33, 34] {
        let original = vec![
            col(Id::TitleCombined),
            col(Id::Artist),
            col(Id::ReleaseYear),
            col(Id::ReleaseDate),
        ];
        let mut result = original.clone();
        let changed = migrate_columns(&mut result, List::Song, View::Table, version).unwrap();
        assert_eq!(changed, version < 29);
        let all = ids(&result);
        assert_eq!(all.contains(&Id::BitDepth), version <= 14);
        assert_eq!(all.contains(&Id::SampleRate), version <= 14);
        assert_eq!(all.contains(&Id::TitleArtist), version <= 20);
        assert_eq!(all.contains(&Id::Composer), version <= 21);
        assert_eq!(all.contains(&Id::AlbumGroup), version <= 26);
        assert_eq!(all.contains(&Id::Year), version < 29);
        if version <= 20 {
            assert_eq!(all.iter().position(|x| *x == Id::TitleArtist), Some(1));
        }
        if version <= 21 {
            assert_eq!(
                all.iter().position(|x| *x == Id::Composer),
                all.iter().position(|x| *x == Id::Artist).map(|i| i + 1)
            );
        }
        if version < 29 {
            assert_eq!(
                all.iter().position(|x| *x == Id::Year).map(|i| i + 1),
                all.iter().position(|x| *x == Id::ReleaseYear)
            );
            assert_eq!(
                all.iter().position(|x| *x == Id::Date).map(|i| i + 1),
                all.iter().position(|x| *x == Id::ReleaseDate)
            );
        }
        for c in original {
            assert!(result.contains(&c));
        }
        if version <= 26 {
            let group = result.iter().find(|c| c.id == Id::AlbumGroup).unwrap();
            assert_eq!(
                (group.pinned, &group.width),
                (Some(Pin::Left), &Number::from(240))
            );
        }
        assert!(!migrate_columns(&mut result, List::Song, View::Table, version).unwrap());
    }
}

#[test]
fn source_lists_views_case_and_empty_arrays() {
    for list in [List::FullScreen, List::SideQueue] {
        let mut result = vec![];
        migrate_columns(&mut result, list, View::Table, 14).unwrap();
        assert!(!ids(&result).contains(&Id::BitDepth));
        assert!(ids(&result).contains(&Id::TitleArtist));
    }
    let mut detail = vec![];
    migrate_columns(&mut detail, List::Album, View::Detail, 28).unwrap();
    assert_eq!(ids(&detail), vec![Id::Year, Id::Date]);
    let mut other = vec![];
    assert!(!migrate_columns(&mut other, List::Song, View::Detail, 14).unwrap());
    assert!(!migrate_columns(&mut other, List::Artist, View::Table, 14).unwrap());
    let mut album = vec![];
    migrate_columns(&mut album, List::Album, View::Table, 14).unwrap();
    assert_eq!(ids(&album), vec![Id::TitleArtist, Id::Composer]);
}

#[test]
fn invalid_input_and_capacity_fail_atomically() {
    let mut duplicate = vec![col(Id::Year), col(Id::Year)];
    let before = duplicate.clone();
    assert!(migrate_columns(&mut duplicate, List::Song, View::Table, 14).is_err());
    assert_eq!(duplicate, before);
    let mut bad = vec![col(Id::Year)];
    bad[0].width = Number::from(16385);
    let before = bad.clone();
    assert!(migrate_columns(&mut bad, List::Song, View::Table, 14).is_err());
    assert_eq!(bad, before);
    // All closed IDs are fewer than 64; a 64-element invalid input still cannot be repaired.
    let mut full = vec![col(Id::Title); 64];
    let before = full.clone();
    assert!(migrate_columns(&mut full, List::Song, View::Table, 14).is_err());
    assert_eq!(full, before);
}

#[test]
fn legacy_autodj_defaults_aliases_and_strict_current() {
    let input: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/p02/settings-migrations/legacy-autodj.json"
    ))
    .unwrap();
    let before = input.clone();
    let result = migrate_auto_dj(&input, 27).unwrap();
    let encoded = serde_json::to_value(result).unwrap();
    assert_eq!(
        encoded,
        json!({"albumStrategy":"similar","songStrategy":"similar","itemCount":3.5,"timing":0.25,"mode":"songs","allowDuplicates":false,"enabled":false,"onlySimilar":false})
    );
    assert_eq!(input, before);
    for version in [14, 15, 20, 21, 22, 26, 27] {
        let result = migrate_auto_dj(
            &json!({"mode":"albums","songStrategy":"library_random","albumStrategy":"unknown"}),
            version,
        )
        .unwrap();
        let v = serde_json::to_value(result).unwrap();
        assert_eq!(v["mode"], "albums");
        assert_eq!(v["songStrategy"], "library_random");
        assert_eq!(v["albumStrategy"], "similar");
    }
    for invalid in [
        json!({"enabled":null}),
        json!({"itemCount":"5"}),
        json!({"extra":1}),
        json!(null),
    ] {
        assert!(migrate_auto_dj(&invalid, 27).is_err());
    }
    for version in [28, 29, 33, 34] {
        assert!(migrate_auto_dj(&input, version).is_err());
        assert!(migrate_auto_dj(&encoded, version).is_ok());
        let mut alias = encoded.clone();
        alias["songStrategy"] = json!("default");
        assert!(migrate_auto_dj(&alias, version).is_err());
    }
}

#[test]
fn table_applicability_matches_each_source_list() {
    let lists = [
        List::Album,
        List::AlbumArtist,
        List::AlbumArtistAlbum,
        List::AlbumArtistSong,
        List::AlbumDetail,
        List::Artist,
        List::Folder,
        List::FullScreen,
        List::Genre,
        List::GenreAlbum,
        List::GenreSong,
        List::Playlist,
        List::PlaylistAlbum,
        List::PlaylistSong,
        List::QueueSong,
        List::Radio,
        List::SideQueue,
        List::Song,
    ];
    for list in lists {
        let mut result = vec![];
        migrate_columns(&mut result, list, View::Table, 14).unwrap();
        let result = ids(&result);
        let v14 = matches!(
            list,
            List::Song | List::PlaylistSong | List::QueueSong | List::AlbumDetail
        );
        let title = v14 || matches!(list, List::Album | List::FullScreen | List::SideQueue);
        let group = matches!(
            list,
            List::Song
                | List::Folder
                | List::PlaylistSong
                | List::AlbumArtistSong
                | List::GenreSong
                | List::QueueSong
                | List::FullScreen
                | List::SideQueue
        );
        assert_eq!(result.contains(&Id::BitDepth), v14, "{list:?}");
        assert_eq!(result.contains(&Id::TitleArtist), title, "{list:?}");
        assert_eq!(result.contains(&Id::Composer), title, "{list:?}");
        assert_eq!(result.contains(&Id::AlbumGroup), group, "{list:?}");
        assert_eq!(
            result.contains(&Id::Date),
            group || list == List::AlbumDetail,
            "{list:?}"
        );
    }
}

#[test]
fn unsupported_versions_reject_without_mutation() {
    use feishin_storage::imports::ImportError;
    for version in [35, u64::MAX] {
        let mut columns = vec![col(Id::TitleCombined)];
        let before = columns.clone();
        assert_eq!(
            migrate_columns(&mut columns, List::Song, View::Table, version),
            Err(ImportError::UnsupportedVersion)
        );
        assert_eq!(columns, before);
        let value = json!({"mode":"albums"});
        let original = value.clone();
        assert_eq!(
            migrate_auto_dj(&value, version),
            Err(ImportError::UnsupportedVersion)
        );
        assert_eq!(value, original);
    }
}

#[test]
fn early_reset_and_transform_boundaries() {
    use feishin_storage::imports::ImportError;
    for version in 0..=34 {
        let expected = match version {
            0..=9 => SettingsPath::EarlyReturn,
            10..=11 => SettingsPath::Reset,
            _ => SettingsPath::Transform,
        };
        assert_eq!(settings_path(version), Ok(expected));
        let mut columns = vec![col(Id::TitleCombined), col(Id::Artist)];
        let before = columns.clone();
        let result = migrate_columns(&mut columns, List::Song, View::Table, version);
        match expected {
            SettingsPath::EarlyReturn => {
                assert_eq!(result, Ok(false));
                assert_eq!(columns, before);
            }
            SettingsPath::Reset => {
                assert_eq!(result, Err(ImportError::SettingsReset));
                assert_eq!(columns, before);
            }
            SettingsPath::Transform if version <= 13 => {
                assert_eq!(result, Ok(true));
                assert!(columns.contains(&before[0]));
                assert!(ids(&columns).contains(&Id::BitDepth));
            }
            SettingsPath::Transform => assert!(result.is_ok()),
        }
    }
    for version in [35, u64::MAX] {
        assert_eq!(settings_path(version), Err(ImportError::UnsupportedVersion));
    }
    let mut duplicate = vec![col(Id::Year), col(Id::Year)];
    let before = duplicate.clone();
    assert_eq!(
        migrate_columns(&mut duplicate, List::Song, View::Table, 9),
        Err(ImportError::InvalidData)
    );
    assert_eq!(duplicate, before);
}

#[test]
fn early_autodj_defaults_without_alias_normalization() {
    use feishin_storage::imports::ImportError;
    let defaults = serde_json::to_value(default_auto_dj()).unwrap();
    assert_eq!(
        defaults,
        json!({"albumStrategy":"similar","allowDuplicates":false,"enabled":false,"itemCount":5,"mode":"songs","onlySimilar":false,"songStrategy":"similar","timing":1})
    );
    for version in 0..=9 {
        let input = json!({"mode":"albums", "itemCount":3.5});
        let before = input.clone();
        let mut expected = defaults.clone();
        expected["mode"] = json!("albums");
        expected["itemCount"] = json!(3.5);
        assert_eq!(
            serde_json::to_value(migrate_auto_dj(&input, version).unwrap()).unwrap(),
            expected
        );
        assert_eq!(input, before);
        for invalid in [
            json!({"mode":"invalid"}),
            json!({"songStrategy":"default"}),
            json!({"albumStrategy":"similar_forward"}),
            json!({"mode":null}),
            json!({"extra":true}),
        ] {
            assert_eq!(
                migrate_auto_dj(&invalid, version),
                Err(ImportError::InvalidData)
            );
        }
    }
    for version in [10, 11] {
        let input = json!({"mode":"albums"});
        let before = input.clone();
        assert_eq!(
            migrate_auto_dj(&input, version),
            Err(ImportError::SettingsReset)
        );
        assert_eq!(input, before);
    }
    for version in [12, 13] {
        let result = migrate_auto_dj(
            &json!({"mode":"invalid", "songStrategy":"default", "albumStrategy":"similar_forward"}),
            version,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), defaults);
    }
}
