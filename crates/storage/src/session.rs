//! Portable single-owner session. Deadlines and cancellation gate operation start only.
use crate::{
    AtomicStore, MAX_BYTES,
    imports::{CopiedInputs, ImportReport, MAX_INPUT_BYTES, MigrationData, SettingsCategory},
    journal::{self, PreparedImport},
    state::{self, PreparedState, StateRepository},
};
use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};
/// Caller must choose a generation unique across all replacement sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionGeneration(pub NonZeroU64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId(pub NonZeroU64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewToken {
    generation: SessionGeneration,
    serial: u64,
}
#[derive(Default)]
pub struct OwnedInputs {
    pub settings: Option<Vec<u8>>,
    pub auth: Option<Vec<u8>>,
    pub player: Option<Vec<u8>>,
    pub separate_queue: Option<Vec<u8>>,
    pub timestamp: Option<Vec<u8>>,
    pub split_categories: BTreeMap<SettingsCategory, Vec<u8>>,
    pub split_settings_version: Option<u64>,
}
impl OwnedInputs {
    fn bounded(&self) -> bool {
        self.settings
            .iter()
            .chain(self.auth.iter())
            .chain(self.player.iter())
            .chain(self.separate_queue.iter())
            .chain(self.timestamp.iter())
            .chain(self.split_categories.values())
            .try_fold(0usize, |n, v| n.checked_add(v.len()))
            .is_some_and(|n| n <= MAX_INPUT_BYTES)
    }
    fn copied(&self) -> CopiedInputs<'_> {
        CopiedInputs {
            settings: self.settings.as_deref(),
            auth: self.auth.as_deref(),
            player: self.player.as_deref(),
            separate_queue: self.separate_queue.as_deref(),
            timestamp: self.timestamp.as_deref(),
            split_categories: self
                .split_categories
                .iter()
                .map(|(k, v)| (*k, v.as_slice()))
                .collect(),
            split_settings_version: self.split_settings_version,
        }
    }
}
pub enum Command {
    Load,
    PrepareSave(Vec<u8>),
    PrepareStage(OwnedInputs),
    PrepareAdoption,
    PrepareRollback,
    Apply(ReviewToken),
    DiscardReview,
}
impl Command {
    pub fn validate_size(&self) -> Result<(), SessionError> {
        match self {
            Self::PrepareSave(b) if b.len() > MAX_BYTES => Err(SessionError::TooLarge),
            Self::PrepareStage(i) if !i.bounded() => Err(SessionError::TooLarge),
            _ => Ok(()),
        }
    }
}
pub struct Request {
    pub generation: SessionGeneration,
    pub id: RequestId,
    pub deadline: Duration,
    pub command: Command,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    WrongGeneration,
    Cancelled,
    Expired,
    TooLarge,
    InvalidSave,
    InvalidToken,
    Exhausted,
    State(state::StateError),
    Journal(journal::JournalError),
}
pub enum Response {
    Load(state::LoadedState),
    ActiveReview {
        token: ReviewToken,
        before: state::Snapshot,
        after: Box<state::Snapshot>,
    },
    StageReview {
        token: ReviewToken,
        data: MigrationData,
        report: ImportReport,
    },
    AppliedActive(state::Snapshot),
    AppliedStage(journal::Snapshot),
    Discarded,
}
pub struct Reply {
    pub generation: SessionGeneration,
    pub id: RequestId,
    pub result: Result<Response, SessionError>,
}
enum Plan {
    Active(Box<PreparedState>),
    Stage(Box<PreparedImport>),
}
pub struct Session<A, J, P> {
    generation: SessionGeneration,
    repository: StateRepository<A, J, P>,
    serial: u64,
    plan: Option<(ReviewToken, Plan)>,
}
impl<A: AtomicStore, J: AtomicStore, P: AtomicStore> Session<A, J, P> {
    pub fn new(generation: SessionGeneration, repository: StateRepository<A, J, P>) -> Self {
        Self {
            generation,
            repository,
            serial: 0,
            plan: None,
        }
    }
    pub fn dispatch(&mut self, request: Request, now: Duration, cancelled: bool) -> Reply {
        let result = if request.generation != self.generation {
            Err(SessionError::WrongGeneration)
        } else if cancelled {
            Err(SessionError::Cancelled)
        } else if now >= request.deadline {
            Err(SessionError::Expired)
        } else {
            request
                .command
                .validate_size()
                .and_then(|()| self.execute(request.command))
        };
        Reply {
            generation: request.generation,
            id: request.id,
            result,
        }
    }
    fn token(&mut self) -> Result<ReviewToken, SessionError> {
        self.serial = self.serial.checked_add(1).ok_or(SessionError::Exhausted)?;
        Ok(ReviewToken {
            generation: self.generation,
            serial: self.serial,
        })
    }
    fn active_review(&mut self, plan: PreparedState) -> Result<Response, SessionError> {
        let token = self.token()?;
        let before = plan.before().clone();
        let after = Box::new(plan.after().clone());
        self.plan = Some((token, Plan::Active(Box::new(plan))));
        Ok(Response::ActiveReview {
            token,
            before,
            after,
        })
    }
    fn execute(&mut self, command: Command) -> Result<Response, SessionError> {
        match command {
            Command::Load => self
                .repository
                .load()
                .map(Response::Load)
                .map_err(SessionError::State),
            Command::PrepareSave(bytes) => {
                let data: MigrationData =
                    serde_json::from_slice(&bytes).map_err(|_| SessionError::InvalidSave)?;
                let p = self
                    .repository
                    .prepare_save(&data)
                    .map_err(SessionError::State)?;
                self.active_review(p)
            }
            Command::PrepareStage(inputs) => {
                let p = self
                    .repository
                    .journal_mut()
                    .prepare(&inputs.copied())
                    .map_err(SessionError::Journal)?;
                let token = self.token()?;
                let data = p.preview().data().clone();
                let report = p.preview().report().clone();
                self.plan = Some((token, Plan::Stage(Box::new(p))));
                Ok(Response::StageReview {
                    token,
                    data,
                    report,
                })
            }
            Command::PrepareAdoption => {
                let p = self
                    .repository
                    .prepare_adoption()
                    .map_err(SessionError::State)?;
                self.active_review(p)
            }
            Command::PrepareRollback => {
                let p = self
                    .repository
                    .prepare_rollback()
                    .map_err(SessionError::State)?;
                self.active_review(p)
            }
            Command::Apply(token) => {
                if self.plan.as_ref().map(|(t, _)| *t) != Some(token) {
                    return Err(SessionError::InvalidToken);
                }
                let Some((_, plan)) = self.plan.take() else {
                    return Err(SessionError::InvalidToken);
                };
                match plan {
                    Plan::Active(p) => self
                        .repository
                        .apply(&p)
                        .map(Response::AppliedActive)
                        .map_err(SessionError::State),
                    Plan::Stage(p) => self
                        .repository
                        .journal_mut()
                        .apply(&p)
                        .map(Response::AppliedStage)
                        .map_err(SessionError::Journal),
                }
            }
            Command::DiscardReview => {
                self.plan = None;
                Ok(Response::Discarded)
            }
        }
    }
}
