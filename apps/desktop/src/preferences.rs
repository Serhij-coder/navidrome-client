use eframe::egui::{self, Context, ThemePreference};
use feishin_storage::{
    Preferences,
    native_worker::{NativeWorker, WorkerError},
    session::{Command, Reply, RequestId, Response, SessionError, SessionGeneration},
    state::{Snapshot, StateError},
};
use std::{num::NonZeroU64, path::PathBuf, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Load,
    Prepare,
    Apply,
}
pub struct Controller {
    worker: NativeWorker,
    generation: SessionGeneration,
    pending: Option<(RequestId, Pending)>,
    confirmed: Option<Snapshot>,
    draft: Preferences,
    status: &'static str,
    closing: bool,
    shutdown: bool,
    stopped: bool,
    terminal: bool,
}
impl Controller {
    pub fn new(root: PathBuf) -> Result<Self, &'static str> {
        let generation = SessionGeneration(NonZeroU64::MIN);
        let worker =
            NativeWorker::open(generation, root, 1).map_err(|_| "Storage worker unavailable")?;
        let mut this = Self {
            worker,
            generation,
            pending: None,
            confirmed: None,
            draft: Preferences::default(),
            status: "Loading preferences",
            closing: false,
            shutdown: false,
            stopped: false,
            terminal: false,
        };
        this.submit(Command::Load, Pending::Load);
        Ok(this)
    }
    fn submit(&mut self, command: Command, kind: Pending) {
        match self.worker.submit(command, Duration::from_secs(30)) {
            Ok(id) => self.pending = Some((id, kind)),
            Err(_) => {
                self.confirmed = None;
                self.closing = false;
                self.status = "Storage unavailable. Reload before saving.";
            }
        }
    }
    fn theme(ctx: &Context, prefs: &Preferences) {
        ctx.set_theme(if prefs.follow_system_theme {
            ThemePreference::System
        } else {
            ThemePreference::Dark
        });
    }
    fn handle_reply(&mut self, ctx: &Context, reply: Reply) {
        if let Some((id, kind)) = self.pending
            && reply.generation == self.generation
            && reply.id == id
        {
            self.pending = None;
            match (kind, reply.result) {
                (Pending::Load, Ok(Response::Load(loaded))) => {
                    self.draft = loaded.snapshot.data.profile.preferences.clone();
                    Self::theme(ctx, &self.draft);
                    self.status = if loaded.recovered.is_empty() {
                        "Preferences loaded"
                    } else {
                        "Some profile data was recovered; review before saving."
                    };
                    self.confirmed = Some(loaded.snapshot);
                }
                (Pending::Prepare, Ok(Response::ActiveReview { token, .. })) => {
                    self.status = "Saving preferences";
                    self.submit(Command::Apply(token), Pending::Apply);
                }
                (Pending::Apply, Ok(Response::AppliedActive(snapshot))) => {
                    self.draft = snapshot.data.profile.preferences.clone();
                    Self::theme(ctx, &self.draft);
                    self.confirmed = Some(snapshot);
                    self.status = "Preferences saved";
                }
                (_, Err(error)) => {
                    self.confirmed = None;
                    self.closing = false;
                    self.status = match error {
                        SessionError::State(StateError::UnsupportedSchema) => {
                            "Saved data uses an unsupported version. Reload after resolving it."
                        }
                        SessionError::State(StateError::InvalidState) => {
                            "Saved data or preferences are invalid. Reload before saving."
                        }
                        SessionError::State(StateError::Conflict) => {
                            "Saved data changed. Reload before saving."
                        }
                        _ => "Storage operation failed. Reload before saving.",
                    };
                }
                _ => {
                    self.confirmed = None;
                    self.closing = false;
                    self.status = "Unexpected storage response. Reload before saving.";
                }
            }
        }
    }
    pub fn tick(&mut self, ctx: &Context) {
        if !self.terminal {
            match self.worker.poll() {
                Ok(Some(reply)) => self.handle_reply(ctx, reply),
                Ok(None) => {}
                Err(_) => {
                    // Unexpected worker exit must remain visible. During an
                    // orderly shutdown, disconnection is expected.
                    if !self.shutdown {
                        self.closing = false;
                    }
                    self.pending = None;
                    self.confirmed = None;
                    self.status = "Storage worker stopped";
                    ctx.request_repaint_after(Duration::from_millis(30));
                    if let Some(result) = self.worker.try_join() {
                        self.terminal = true;
                        self.status = match result {
                            Err(WorkerError::Factory(_)) => "Storage could not be opened",
                            Err(WorkerError::Panicked) => "Storage worker failed",
                            _ => "Storage worker stopped",
                        };
                    }
                }
            }
        }
        if self.closing && self.terminal {
            self.stopped = true;
        }
        if self.closing && self.pending.is_none() && !self.shutdown && !self.terminal {
            self.worker.begin_shutdown();
            self.shutdown = true;
        }
        if self.shutdown
            && let Some(result) = self.worker.try_join()
        {
            self.stopped = true;
            if result.is_err() {
                self.status = "Storage worker stopped with an error";
            }
        }
        if self.pending.is_some() || (self.closing && !self.stopped) {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
    pub fn close(&mut self) {
        self.closing = true;
    }
    pub fn stopped(&self) -> bool {
        self.stopped
    }
    pub fn show(&mut self, ctx: &Context) {
        egui::Window::new("Preferences").show(ctx, |ui| {
            if let Some(snapshot) = &self.confirmed {
                let p = &snapshot.data.profile;
                let name = p
                    .current_server
                    .as_ref()
                    .and_then(|id| p.profiles.iter().find(|p| &p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("None");
                ui.label(format!(
                    "Profiles: {} | Current: {}",
                    p.profiles.len(),
                    name
                ));
            } else {
                ui.label("Profile summary unavailable");
            }
            ui.label("The demo table is not connected to saved profiles.");
            let language_label = ui.label("Preferred language");
            ui.add_enabled(
                !self.closing && self.pending.is_none(),
                egui::TextEdit::singleline(&mut self.draft.language).char_limit(64),
            )
            .labelled_by(language_label.id);
            ui.label("Language is saved only; translations are unavailable.");
            ui.add_enabled(
                !self.closing && self.pending.is_none(),
                egui::Checkbox::new(
                    &mut self.draft.follow_system_theme,
                    "Follow system appearance",
                ),
            );
            ui.horizontal(|ui| {
                // The pinned AT-SPI adapter reports disabled buttons as enabled.
                // Expose an action only when saving is actually available.
                if !self.closing && self.pending.is_none() && self.confirmed.is_some() {
                    if ui.button("Save").clicked()
                        && let Some(snapshot) = &self.confirmed
                    {
                        self.status = "Preparing save";
                        self.submit(
                            Command::PreparePreferences {
                                expected_revision: snapshot.revision,
                                preferences: self.draft.clone(),
                            },
                            Pending::Prepare,
                        );
                    }
                } else {
                    ui.label("Save unavailable");
                }
                if ui
                    .add_enabled(
                        !self.closing && !self.terminal && self.pending.is_none(),
                        egui::Button::new("Reload"),
                    )
                    .clicked()
                {
                    self.confirmed = None;
                    self.status = "Loading preferences";
                    self.submit(Command::Load, Pending::Load);
                }
            });
            ui.label(if self.closing && !self.stopped {
                "Waiting for storage to finish before closing"
            } else {
                self.status
            });
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/desktop/preferences.rs"]
mod tests;
