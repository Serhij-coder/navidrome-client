//! Test-only WASM export; not linked into either application shell.
#[cfg(target_arch = "wasm32")]
#[path = "state_browser_probe.rs"]
mod state_probe;

#[cfg(target_arch = "wasm32")]
mod probe {
    use feishin_storage::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn storage_probe(action: &str) -> Result<bool, JsValue> {
        let result = run(action).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(result)
    }
    fn run(action: &str) -> Result<bool, Error> {
        if action.starts_with("state-") {
            return super::state_probe::run(action).map_err(|_| Error::InvalidDocument);
        }
        if action.starts_with("import-") {
            return import_run(action).map_err(|_| Error::InvalidDocument);
        }
        let mut repo = ProfileRepository::new(BrowserStore::open()?);
        match action {
            "defaults" => Ok(repo.load()?.data == ProfileData::default()),
            "save" => {
                let data = ProfileData {
                    preferences: Preferences {
                        language: "uk".into(),
                        follow_system_theme: true,
                    },
                    ..ProfileData::default()
                };
                repo.save(&data)?;
                Ok(repo.load()?.data == data)
            }
            "reload" => Ok(repo.load()?.data.preferences.language == "uk"),
            "partial" => {
                let loaded = repo.load()?;
                Ok(loaded.data.preferences == Preferences::default()
                    && loaded.recovered == vec![Recovery::Language])
            }
            "reject-save" => {
                repo.save(&ProfileData::default())?;
                Ok(true)
            }
            _ => Err(Error::InvalidDocument),
        }
    }
    fn import_run(action: &str) -> Result<bool, journal::JournalError> {
        use imports::CopiedInputs;
        use journal::{JournalError, JournalRepository};
        let mut repo = JournalRepository::new(BrowserStore::open_imports()?);
        let uk = CopiedInputs {
            settings: Some(
                br#"{"version":34,"general":{"language":"uk","followSystemTheme":true}}"#,
            ),
            ..CopiedInputs::default()
        };
        let en = CopiedInputs {
            settings: Some(
                br#"{"version":34,"general":{"language":"en","followSystemTheme":false}}"#,
            ),
            ..CopiedInputs::default()
        };
        match action {
            "import-defaults" => Ok(repo.load()?.data.is_none()),
            "import-save" => {
                let prepared = repo.prepare(&uk)?;
                let before = repo.load()?;
                let applied = repo.apply(&prepared)?;
                Ok(applied.revision > before.revision
                    && applied
                        .data
                        .as_ref()
                        .is_some_and(|d| d.profile.preferences.language == "uk"))
            }
            "import-reload" => Ok(repo
                .load()?
                .data
                .as_ref()
                .is_some_and(|d| d.profile.preferences.language == "uk")),
            "import-rollback" => {
                let prepared = repo.prepare_rollback()?;
                Ok(repo.rollback(&prepared)?.data.is_none())
            }
            "import-redo" => {
                let prepared = repo.prepare_rollback()?;
                Ok(repo
                    .rollback(&prepared)?
                    .data
                    .as_ref()
                    .is_some_and(|d| d.profile.preferences.language == "uk"))
            }
            "import-reset-save" => {
                let raw = br#"{"version":10,"state":{"general":{"sidebarItems":[]},"lists":null}}"#;
                let prepared = repo.prepare(&CopiedInputs {
                    settings: Some(raw),
                    ..Default::default()
                })?;
                let reported = prepared
                    .preview()
                    .report()
                    .diagnostics
                    .iter()
                    .any(|d| d.code == imports::Code::SettingsReset);
                let expected = prepared.preview().data().clone();
                let applied = repo.apply(&prepared)?;
                Ok(reported && applied.data == Some(expected))
            }
            "import-reset-reload" => Ok(repo.load()?.data.as_ref().is_some_and(|d| {
                d.profile.preferences == Preferences::default()
                    && d.columns.is_empty()
                    && d.auto_dj.as_ref().is_some_and(|auto| {
                        !auto.enabled && auto.mode == imports::AutoDjMode::Songs
                    })
            })),
            "import-reset-undo" => {
                let prepared = repo.prepare_rollback()?;
                let expected = prepared.restored().cloned();
                let restored = repo.rollback(&prepared)?;
                Ok(restored.data == expected
                    && restored
                        .data
                        .as_ref()
                        .is_some_and(|d| d.profile.preferences.language == "uk"))
            }
            "import-reset-malformed" => {
                let before = repo.load()?;
                let raw = br#"{"version":10,"state":{"general":{}}}"#;
                let rejected = matches!(
                    repo.prepare(&CopiedInputs {
                        settings: Some(raw),
                        ..Default::default()
                    }),
                    Err(JournalError::Import(imports::ImportError::InvalidDocument))
                );
                Ok(rejected && repo.load()? == before)
            }
            "import-historical" => {
                let raw = br#"{"version":20,"state":{"lists":{"song":{"table":{"columns":[{"id":"titleCombined","align":"end","autoSize":false,"isEnabled":true,"pinned":null,"width":73}]}}},"autoDJ":{"mode":"old","songStrategy":"similar_forward"}}}"#;
                let input = CopiedInputs {
                    settings: Some(raw),
                    ..CopiedInputs::default()
                };
                let before = repo.load()?;
                let prepared = repo.prepare(&input)?;
                let reviewed = prepared.preview().data().clone();
                let expected = reviewed.columns.iter().any(|list| {
                    list.columns
                        .iter()
                        .any(|c| c.id == imports::ColumnId::TitleArtist)
                }) && reviewed
                    .auto_dj
                    .as_ref()
                    .is_some_and(|d| d.song_strategy == imports::AutoDjStrategy::Similar);
                let applied = repo.apply(&prepared)?;
                let repeated = repo.prepare(&input)?;
                let unchanged = repo.apply(&repeated)? == applied;
                let restored = repo.prepare_rollback()?;
                Ok(expected
                    && applied.data == Some(reviewed)
                    && unchanged
                    && repo.rollback(&restored)?.data == before.data)
            }
            "import-corrupt-current" => {
                let input = CopiedInputs {
                    player: Some(br#"{"version":4,"state":{"player":{"shuffle":"none","index":1},"queue":{"default":["missing","first","second"],"priority":[],"shuffled":[],"songs":{"first":{"id":"song","_serverId":"server","name":"First","duration":5000},"second":{"id":"song","_serverId":"server","name":"Second","duration":5000}}}}}"#),
                    timestamp: Some(b"1.25"),
                    ..CopiedInputs::default()
                };
                let preview = imports::preview(&input, &Default::default())?;
                Ok(preview.data().queue.as_ref().is_some_and(|queue| {
                    queue.occurrences.len() == 2
                        && queue.current_occurrence.is_none()
                        && queue.position.is_none()
                }) && preview
                    .report()
                    .diagnostics
                    .iter()
                    .any(|d| d.code == imports::Code::CurrentRejected)
                    && preview
                        .report()
                        .diagnostics
                        .iter()
                        .any(|d| d.code == imports::Code::TimestampRejected))
            }
            "import-stale" => {
                let stale = repo.prepare(&uk)?;
                let newer = repo.prepare(&en)?;
                repo.apply(&newer)?;
                let rejected = repo.apply(&stale) == Err(JournalError::Conflict);
                let restored = repo.prepare_rollback()?;
                repo.rollback(&restored)?;
                Ok(rejected)
            }
            _ => Err(JournalError::InvalidJournal),
        }
    }
}
