//! Test-only active-state adoption operations, executed by the browser harness.
use feishin_storage::{imports::CopiedInputs, state::*, *};

pub fn run(action: &str) -> Result<bool, StateError> {
    let mut repo = StateRepository::new(
        BrowserStore::open_state()?,
        BrowserStore::open_imports()?,
        BrowserStore::open()?,
    );
    match action {
        "state-defaults" => {
            let loaded = repo.load()?;
            Ok(loaded.snapshot.revision == 0
                && loaded.snapshot.data.profile.preferences.language == "de")
        }
        "state-adopt" => {
            let input = CopiedInputs {
                settings: Some(
                    br#"{"version":34,"general":{"language":"uk","followSystemTheme":true}}"#,
                ),
                auth: Some(include_bytes!("../fixtures/p02/imports/auth-v2.json")),
                player: Some(include_bytes!("../fixtures/p02/imports/player-v3.json")),
                timestamp: Some(b"12.5"),
                ..Default::default()
            };
            let stage = repo.journal_mut().prepare(&input)?;
            repo.journal_mut().apply(&stage)?;
            let before = repo.load()?.snapshot;
            let prepared = repo.prepare_adoption()?;
            let after = prepared.after().data.clone();
            let readonly = repo.load()?.snapshot == before;
            let adopted = repo.apply(&prepared)?;
            Ok(readonly
                && adopted.data == after
                && adopted.data.profile.preferences.language == "uk"
                && adopted.data.queue.as_ref().is_some_and(|q| {
                    q.occurrences.len() == 2
                        && q.occurrences[0].song_id == q.occurrences[1].song_id
                        && q.occurrences[0].occurrence_id != q.occurrences[1].occurrence_id
                }))
        }
        "state-reload" => {
            let loaded = repo.load()?.snapshot;
            Ok(loaded.revision > 0
                && loaded.data.profile.preferences.language == "uk"
                && loaded
                    .data
                    .queue
                    .as_ref()
                    .is_some_and(|q| q.occurrences.len() == 2))
        }
        "state-undo-first" => {
            let prepared = repo.prepare_rollback()?;
            let restored = repo.apply(&prepared)?;
            Ok(restored.data.profile.preferences.language == "de" && restored.data.queue.is_none())
        }
        "state-redo" => {
            let prepared = repo.prepare_rollback()?;
            Ok(repo.apply(&prepared)?.data.profile.preferences.language == "uk")
        }
        "state-idempotent" => {
            let before = repo.load()?.snapshot;
            let prepared = repo.prepare_adoption()?;
            Ok(repo.apply(&prepared)? == before)
        }
        "state-save" => {
            let mut data = repo.load()?.snapshot.data;
            data.profile.preferences.language = if data.profile.preferences.language == "fr" {
                "it"
            } else {
                "fr"
            }
            .into();
            let prepared = repo.prepare_save(&data)?;
            Ok(repo.apply(&prepared)?.data == data)
        }
        "state-stale-target" => {
            let prepared = repo.prepare_adoption()?;
            let mut data = repo.load()?.snapshot.data;
            data.profile.preferences.language = "es".into();
            let local = repo.prepare_save(&data)?;
            let saved = repo.apply(&local)?;
            Ok(matches!(repo.apply(&prepared), Err(StateError::Conflict))
                && repo.load()?.snapshot == saved)
        }
        "state-stale-stage" => {
            let prepared = repo.prepare_adoption()?;
            let before = repo.load()?.snapshot;
            let input = CopiedInputs {
                settings: Some(br#"{"version":34,"general":{"language":"en"}}"#),
                ..Default::default()
            };
            let stage = repo.journal_mut().prepare(&input)?;
            repo.journal_mut().apply(&stage)?;
            Ok(matches!(repo.apply(&prepared), Err(StateError::Conflict))
                && repo.load()?.snapshot == before)
        }
        "state-rollback" => {
            let prepared = repo.prepare_rollback()?;
            let after = prepared.after().data.clone();
            Ok(repo.apply(&prepared)?.data == after)
        }
        _ => Err(StateError::Conflict),
    }
}
