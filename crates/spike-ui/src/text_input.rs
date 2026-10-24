//! Narrow compatibility treatment for egui 0.33.3's preedit cursor guard.
use egui::{Context, Event, Id, ImeEvent, Key};

#[derive(Default)]
pub(super) struct CompositionGuard {
    owner: Option<Id>,
    preedit: bool,
}

impl CompositionGuard {
    pub(super) fn prepare(&mut self, ctx: &Context, editor: Id) {
        self.finish(ctx);
        if ctx.memory(|memory| memory.focused()) != Some(editor)
            || !ctx.input(|input| input.focused)
        {
            return;
        }
        if self.owner != Some(editor) {
            self.owner = Some(editor);
            self.preedit = false;
        }
        ctx.input_mut(|input| {
            let mut editor_input = true;
            // Keep event order: a Commit/Disabled before Home in this same frame
            // must permit normal cursor movement. Do not replace IME/text events.
            input.events.retain(|event| match event {
                // A pointer press can move the cursor or transfer focus later in
                // this UI pass. Stop guarding now so a following table key is
                // never removed using the previous frame's editor ownership.
                Event::WindowFocused(false)
                | Event::PointerButton { pressed: true, .. }
                | Event::Key {
                    key: Key::Tab | Key::Escape,
                    pressed: true,
                    ..
                } => {
                    self.owner = None;
                    self.preedit = false;
                    editor_input = false;
                    true
                }
                Event::Ime(ImeEvent::Preedit(text)) => {
                    // egui ignores newline predictions without changing its range.
                    if editor_input && text != "\n" && text != "\r" {
                        self.preedit = !text.is_empty();
                    }
                    true
                }
                Event::Ime(ImeEvent::Commit(text)) if text == "\n" || text == "\r" => true,
                Event::Ime(ImeEvent::Enabled | ImeEvent::Commit(_) | ImeEvent::Disabled) => {
                    self.preedit = false;
                    true
                }
                // egui already filters arrows during IME, but Home/End can move
                // its selected preedit range and cause Commit to drop replacement.
                Event::Key {
                    key: Key::Home | Key::End,
                    ..
                } => !self.preedit,
                _ => true,
            });
        });
    }

    pub(super) fn finish(&mut self, ctx: &Context) {
        if self.owner != ctx.memory(|memory| memory.focused()) || !ctx.input(|input| input.focused)
        {
            self.owner = None;
            self.preedit = false;
        }
    }
}
