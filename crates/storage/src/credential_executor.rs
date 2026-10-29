//! Portable credential dispatch. Cancellation and deadlines gate start only.
use crate::credentials::{
    CredentialError, CredentialKey, CredentialOwner, MAX_SECRET_BYTES, NoPersistence,
    PersistentCredentials,
};
use feishin_domain::Secret;
use std::{num::NonZeroU64, time::Duration};

/// Choose a fresh generation for each replacement owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(pub NonZeroU64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId(pub NonZeroU64);

pub enum Command {
    Put(CredentialKey, Secret),
    Get(CredentialKey),
    Clear,
    Remember(CredentialKey),
    Restore(CredentialKey),
    Forget(CredentialKey),
}
impl Command {
    pub fn validate_size(&self) -> Result<(), DispatchError> {
        if matches!(self, Self::Put(_, secret) if secret.expose().len() > MAX_SECRET_BYTES) {
            Err(DispatchError::TooLarge)
        } else {
            Ok(())
        }
    }
}
pub struct Request {
    pub generation: Generation,
    pub id: RequestId,
    pub latest_start: Duration,
    pub command: Command,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    WrongGeneration,
    Cancelled,
    Expired,
    TooLarge,
    Operation(CredentialError),
    /// A provider mutation may have taken effect; reconcile before retrying.
    MutationUncertain(CredentialError),
}
pub enum Response {
    Done,
    Secret(Option<Secret>),
    Restored(bool),
}
pub struct Reply {
    pub generation: Generation,
    pub id: RequestId,
    pub result: Result<Response, DispatchError>,
}
pub struct Dispatcher<B = NoPersistence> {
    generation: Generation,
    owner: CredentialOwner<B>,
}
impl Dispatcher<NoPersistence> {
    pub fn new(generation: Generation) -> Self {
        Self::with_backend(generation, NoPersistence)
    }
}
impl<B: PersistentCredentials> Dispatcher<B> {
    pub fn with_backend(generation: Generation, backend: B) -> Self {
        Self {
            generation,
            owner: CredentialOwner::with_backend(backend),
        }
    }
    pub fn dispatch(&mut self, request: Request, now: Duration, cancelled: bool) -> Reply {
        let result = if request.generation != self.generation {
            Err(DispatchError::WrongGeneration)
        } else if cancelled {
            Err(DispatchError::Cancelled)
        } else if now >= request.latest_start {
            Err(DispatchError::Expired)
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
    fn execute(&mut self, command: Command) -> Result<Response, DispatchError> {
        let mutation = matches!(command, Command::Remember(_) | Command::Forget(_));
        let result = match command {
            Command::Put(key, secret) => {
                self.owner.put_session(key, secret).map(|()| Response::Done)
            }
            Command::Get(key) => Ok(Response::Secret(
                self.owner
                    .session(&key)
                    .map(|s| Secret::new(s.expose().to_vec())),
            )),
            Command::Clear => {
                self.owner.clear_session();
                Ok(Response::Done)
            }
            Command::Remember(key) => self.owner.remember(&key).map(|()| Response::Done),
            Command::Restore(key) => self.owner.restore_remembered(&key).map(Response::Restored),
            Command::Forget(key) => self.owner.forget(&key).map(|()| Response::Done),
        };
        result.map_err(|e| {
            if mutation {
                DispatchError::MutationUncertain(e)
            } else {
                DispatchError::Operation(e)
            }
        })
    }
}
