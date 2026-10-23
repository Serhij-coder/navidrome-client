use super::*;

pub(super) struct TrackActions {
    track: usize,
    generation: u64,
    focus_menu: bool,
    copy_id: Option<egui::Id>,
    serial: u64,
}

impl SpikeApp {
    pub(super) fn show_track_actions(&mut self, ui: &mut egui::Ui, table: &egui::Response) {
        let ctx = ui.ctx().clone();
        ctx.accesskit_node_builder(table.id, |node| {
            node.set_description("Shift+F10 opens actions for the focused track.");
        });
        if table.has_focus()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F10))
            && !self.pending
            && let Some(track) = self.selection.focus
            // Membership is checked on this user command, not on every frame.
            && self.rows.contains(&ViewRow::Track(track))
        {
            egui::Popup::close_all(&ctx);
            self.track_action_serial = self.track_action_serial.wrapping_add(1);
            self.track_actions = Some(TrackActions {
                track,
                generation: self.generation,
                focus_menu: true,
                copy_id: None,
                serial: self.track_action_serial,
            });
            ctx.request_repaint();
        }
        let Some(mut action) = self.track_actions.take() else {
            return;
        };
        if self.pending
            || action.generation != self.generation
            || self.selection.focus != Some(action.track)
        {
            // Preserve focus on an outside input, but release a now-hidden menu.
            if action.copy_id.is_some() && ctx.memory(|memory| memory.focused()) == action.copy_id {
                table.request_focus();
            }
            ctx.request_repaint();
            return;
        }
        let escape = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        let mut open = true;
        let mut copied = false;
        egui::Popup::menu(table)
            .id(table.id.with("focused-track-actions"))
            .open_bool(&mut open)
            .show(|ui| {
                let title = &self.data[action.track].title;
                ui.label(title);
                // A dismissed menu item is a new accessibility object next time.
                // Keep the Area ID stable; only this stateless button ID changes.
                let copy = ui
                    .push_id(action.serial, |ui| ui.button("Copy synthetic title"))
                    .inner;
                action.copy_id = Some(copy.id);
                if action.focus_menu && ui.is_visible() {
                    copy.request_focus();
                    action.focus_menu = false;
                    ctx.request_repaint();
                }
                if copy.clicked() {
                    ctx.copy_text(title.clone());
                    copied = true;
                    ui.close();
                }
            });
        if !open || copied {
            if escape || copied {
                table.request_focus();
            }
            ctx.request_repaint();
        } else {
            self.track_actions = Some(action);
        }
    }
}
