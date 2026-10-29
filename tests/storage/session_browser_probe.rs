//! Local browser dispatcher fixture. No Web Worker/off-thread claim.
use feishin_storage::{session::*, state::StateRepository, *};
use std::{num::NonZeroU64, time::Duration};

type BrowserSession = Session<BrowserStore, BrowserStore, BrowserStore>;

fn send(
    session: &mut BrowserSession,
    generation: SessionGeneration,
    id: u64,
    command: Command,
) -> Result<Response, SessionError> {
    let id = RequestId(NonZeroU64::new(id).ok_or(SessionError::Exhausted)?);
    let reply = session.dispatch(
        Request {
            generation,
            id,
            deadline: Duration::from_secs(10),
            command,
        },
        Duration::ZERO,
        false,
    );
    if reply.generation != generation || reply.id != id {
        return Err(SessionError::WrongGeneration);
    }
    reply.result
}
fn active_token(response: Response) -> Result<ReviewToken, SessionError> {
    match response {
        Response::ActiveReview { token, .. } => Ok(token),
        _ => Err(SessionError::InvalidToken),
    }
}
fn loaded(response: Response) -> Result<feishin_storage::state::Snapshot, SessionError> {
    match response {
        Response::Load(data) => Ok(data.snapshot),
        _ => Err(SessionError::InvalidToken),
    }
}
pub fn run(action: &str) -> Result<bool, SessionError> {
    let serial = match action {
        "session-flow" => 1,
        "session-reload" => 2,
        "session-controls" => 3,
        "session-bounds" => 4,
        "session-discard" => 5,
        "session-write-error" => 6,
        _ => return Err(SessionError::InvalidToken),
    };
    let generation = SessionGeneration(NonZeroU64::new(serial).ok_or(SessionError::Exhausted)?);
    let mut session = Session::new(
        generation,
        StateRepository::new(
            BrowserStore::open_state().map_err(|e| SessionError::State(e.into()))?,
            BrowserStore::open_imports().map_err(|e| SessionError::State(e.into()))?,
            BrowserStore::open().map_err(|e| SessionError::State(e.into()))?,
        ),
    );
    match action {
        "session-flow" => {
            let before = loaded(send(&mut session, generation, 1, Command::Load)?)?;
            let response = send(
                &mut session,
                generation,
                2,
                Command::PrepareStage(OwnedInputs {
                    settings: Some(br#"{"version":34,"general":{"language":"uk"}}"#.to_vec()),
                    auth: Some(include_bytes!("../fixtures/p02/imports/auth-v2.json").to_vec()),
                    player: Some(include_bytes!("../fixtures/p02/imports/player-v3.json").to_vec()),
                    timestamp: Some(b"12.5".to_vec()),
                    ..Default::default()
                }),
            )?;
            let Response::StageReview {
                token,
                data,
                report,
            } = response
            else {
                return Ok(false);
            };
            let pure = loaded(send(&mut session, generation, 3, Command::Load)?)? == before;
            let staged = matches!(
                send(&mut session, generation, 4, Command::Apply(token))?,
                Response::AppliedStage(_)
            );
            let consumed = matches!(
                send(&mut session, generation, 5, Command::Apply(token)),
                Err(SessionError::InvalidToken)
            );
            let old = active_token(send(&mut session, generation, 6, Command::PrepareAdoption)?)?;
            let latest =
                active_token(send(&mut session, generation, 7, Command::PrepareAdoption)?)?;
            let superseded = matches!(
                send(&mut session, generation, 8, Command::Apply(old)),
                Err(SessionError::InvalidToken)
            );
            let Response::AppliedActive(adopted) =
                send(&mut session, generation, 9, Command::Apply(latest))?
            else {
                return Ok(false);
            };
            let undo = active_token(send(
                &mut session,
                generation,
                10,
                Command::PrepareRollback,
            )?)?;
            let Response::AppliedActive(restored) =
                send(&mut session, generation, 11, Command::Apply(undo))?
            else {
                return Ok(false);
            };
            let redo = active_token(send(
                &mut session,
                generation,
                12,
                Command::PrepareRollback,
            )?)?;
            let Response::AppliedActive(redone) =
                send(&mut session, generation, 13, Command::Apply(redo))?
            else {
                return Ok(false);
            };
            Ok(pure
                && staged
                && consumed
                && superseded
                && report.partial
                && adopted.data == data
                && restored.data == before.data
                && redone.data == data
                && redone.revision == 3
                && data.queue.as_ref().is_some_and(|q| {
                    q.occurrences.len() == 2
                        && q.occurrences[0].song_id == q.occurrences[1].song_id
                        && q.occurrences[0].occurrence_id != q.occurrences[1].occurrence_id
                }))
        }
        "session-reload" => {
            let data = loaded(send(&mut session, generation, 1, Command::Load)?)?;
            Ok(data.revision == 3 && data.data.profile.preferences.language == "uk")
        }
        "session-controls" => {
            let before = loaded(send(&mut session, generation, 1, Command::Load)?)?;
            let request = |g, command| Request {
                generation: g,
                id: RequestId(NonZeroU64::MIN),
                deadline: Duration::from_secs(1),
                command,
            };
            let cancelled = session
                .dispatch(request(generation, Command::Load), Duration::ZERO, true)
                .result;
            let expired = session
                .dispatch(
                    request(generation, Command::Load),
                    Duration::from_secs(1),
                    false,
                )
                .result;
            let wrong = session
                .dispatch(
                    request(SessionGeneration(NonZeroU64::MIN), Command::Load),
                    Duration::ZERO,
                    false,
                )
                .result;
            Ok(matches!(cancelled, Err(SessionError::Cancelled))
                && matches!(expired, Err(SessionError::Expired))
                && matches!(wrong, Err(SessionError::WrongGeneration))
                && loaded(send(&mut session, generation, 2, Command::Load)?)? == before)
        }
        "session-bounds" => {
            let too_big = send(
                &mut session,
                generation,
                1,
                Command::PrepareSave(vec![0; MAX_BYTES + 1]),
            );
            let malformed = send(
                &mut session,
                generation,
                2,
                Command::PrepareSave(b"{bad".to_vec()),
            );
            let split = OwnedInputs {
                split_categories: [
                    (
                        imports::SettingsCategory::General,
                        vec![0; imports::MAX_INPUT_BYTES / 2 + 1],
                    ),
                    (
                        imports::SettingsCategory::Playback,
                        vec![0; imports::MAX_INPUT_BYTES / 2 + 1],
                    ),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            };
            let aggregate = send(&mut session, generation, 3, Command::PrepareStage(split));
            Ok(matches!(too_big, Err(SessionError::TooLarge))
                && matches!(malformed, Err(SessionError::InvalidSave))
                && matches!(aggregate, Err(SessionError::TooLarge)))
        }
        "session-discard" | "session-write-error" => {
            let before = loaded(send(&mut session, generation, 1, Command::Load)?)?;
            let mut data = before.data.clone();
            data.profile.preferences.language = "fr".into();
            let bytes = serde_json::to_vec(&data).map_err(|_| SessionError::InvalidSave)?;
            let token = active_token(send(
                &mut session,
                generation,
                2,
                Command::PrepareSave(bytes),
            )?)?;
            let rejected = if action == "session-discard" {
                send(&mut session, generation, 3, Command::DiscardReview)?;
                matches!(
                    send(&mut session, generation, 4, Command::Apply(token)),
                    Err(SessionError::InvalidToken)
                )
            } else {
                matches!(
                    send(&mut session, generation, 4, Command::Apply(token)),
                    Err(SessionError::State(state::StateError::Storage(
                        Error::Unavailable
                    )))
                )
            };
            Ok(rejected
                && matches!(
                    send(&mut session, generation, 5, Command::Apply(token)),
                    Err(SessionError::InvalidToken)
                )
                && loaded(send(&mut session, generation, 6, Command::Load)?)? == before)
        }
        _ => Err(SessionError::InvalidToken),
    }
}
