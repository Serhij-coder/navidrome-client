//! P01 feasibility surface. No server, profile, production queue or persistence.
use egui::{Color32, RichText, Sense, Vec2};
use feishin_spike_model::{
    Analysis, Query, Record, Selection, ViewRow, build_view, css_report, records,
};
use std::{sync::Arc, time::Duration};

pub const ROW_HEIGHT: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Title,
    Album,
    Identity,
}
#[derive(Clone)]
struct ColumnDrag(Column);
#[derive(Clone)]
struct TrackDrag(usize);

#[cfg(not(target_arch = "wasm32"))]
mod artwork;
#[cfg(not(target_arch = "wasm32"))]
mod worker;

pub struct SpikeApp {
    data: Arc<Vec<Record>>,
    rows: Arc<Vec<ViewRow>>,
    query: Query,
    selection: Selection,
    columns: [Column; 3],
    widths: [f32; 3],
    generation: u64,
    pending: bool,
    remote: bool,
    css: String,
    rtl: bool,
    visualizer: bool,
    pub analysis: Option<Analysis>,
    pub notice: Option<&'static str>,
    pub rows_drawn: usize,
    pub max_row_height: f32,
    pub frames: u64,
    table_focus: bool,
    #[cfg(test)]
    test_keyboard: Option<egui::Response>,
    #[cfg(test)]
    test_filter: Option<egui::Response>,
    scroll_to: Option<usize>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: worker::Worker,
    #[cfg(not(target_arch = "wasm32"))]
    artwork: artwork::Artwork,
    artwork_enabled: bool,
}

impl SpikeApp {
    pub fn new(_ctx: &egui::Context, remote: bool) -> Self {
        let count = if remote {
            100
        } else if cfg!(target_arch = "wasm32") {
            1000
        } else {
            100_000
        };
        let data = records(count);
        let rows = Arc::new(build_view(&data, &Query::default(), || false).unwrap_or_default());
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            worker: worker::Worker::new(data.clone(), _ctx.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            artwork: artwork::Artwork::new(_ctx.clone()),
            artwork_enabled: false,
            data,
            rows,
            query: Query::default(),
            selection: Selection::default(),
            columns: [Column::Title, Column::Album, Column::Identity],
            widths: [280.0, 160.0, 100.0],
            generation: 0,
            pending: false,
            remote,
            css: String::new(),
            rtl: false,
            visualizer: false,
            analysis: None,
            notice: None,
            rows_drawn: 0,
            max_row_height: 0.0,
            frames: 0,
            table_focus: false,
            #[cfg(test)]
            test_keyboard: None,
            #[cfg(test)]
            test_filter: None,
            scroll_to: None,
        }
    }

    pub fn set_artwork_visible(&mut self, visible: bool) {
        self.artwork_enabled = visible;
    }

    pub fn wants_analysis(&self) -> bool {
        self.visualizer
    }
    pub fn set_analysis_visible(&mut self, visible: bool) {
        self.visualizer = visible;
    }
    pub fn enable_analysis(&mut self) {
        self.visualizer = true;
    }

    fn submit(&mut self, ctx: &egui::Context) {
        self.generation += 1;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending = true;
            self.worker.submit(self.generation, self.query.clone());
        }
        #[cfg(target_arch = "wasm32")]
        {
            // Web shell deliberately bounds the startup/interactive fixture to 1k.
            // A browser worker and the full 100k workload remain a P01 evidence gate.
            self.rows = Arc::new(build_view(&self.data, &self.query, || false).unwrap_or_default());
        }
        ctx.request_repaint();
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        self.rows_drawn = 0;
        self.max_row_height = 0.0;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((generation, rows)) = self.worker.take()
            && generation == self.generation
        {
            self.rows = Arc::new(rows);
            self.pending = false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.artwork.receive(ctx);
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.heading(if self.remote { "Remote feasibility shell" } else { "Feishin Rust - P01 feasibility" });
            if let Some(notice) = self.notice { ui.colored_label(Color32::YELLOW, notice); }
            ui.label("Synthetic records only. No server login, saved profile or production playback.");
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                let filter = ui.add(egui::TextEdit::singleline(&mut self.query.text).char_limit(256)).on_hover_text("Filter track title");
                #[cfg(test)]
                { self.test_filter = Some(filter.clone()); }
                changed |= filter.changed();
                changed |= ui.checkbox(&mut self.query.descending, "Descending").changed();
                changed |= ui.checkbox(&mut self.query.grouped, "Album groups").changed();
                if ui.button("Reset order").clicked() { self.query.move_before = None; changed = true; }
                ui.toggle_value(&mut self.rtl, "RTL layout");
                ui.toggle_value(&mut self.visualizer, "Analysis preset");
                ui.add_enabled(!cfg!(target_arch = "wasm32"), egui::Checkbox::new(&mut self.artwork_enabled, "Owned artwork churn"));
                egui::widgets::global_theme_preference_buttons(ui);
            });
            if changed { self.query.move_before = None; self.submit(ctx); }
            ui.horizontal(|ui| {
                ui.label("Scale");
                let mut scale = ctx.zoom_factor();
                if ui.add(egui::Slider::new(&mut scale, 0.75..=2.0)).changed() { ctx.set_zoom_factor(scale); }
                ui.label(format!("{} rows, {} selected", self.rows.len(), self.selection.selected.len()));
                if self.pending { ui.label("Building latest view..."); }
            });
            ui.collapsing("Column widths / CSS replacement report / text input", |ui| {
                for (index, width) in self.widths.iter_mut().enumerate() {
                    ui.add(egui::Slider::new(width, 70.0..=500.0).text(format!("Column {}", index + 1)));
                }
                ui.add(egui::TextEdit::multiline(&mut self.css).char_limit(65_536));
                ui.label(css_report(&self.css));
                ui.label("IME / RTL probe: type your own text above. Fonts here are egui defaults; full script coverage is unverified.");
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.visualizer {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
                if let Some(frame) = &self.analysis {
                    paint_preset(ui.painter(), rect, frame);
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No PCM source connected. Use native capture probe or browser analyser.",
                        egui::FontId::proportional(14.0),
                        ui.visuals().text_color(),
                    );
                }
            }
            let keyboard_button = ui.button("Focus table (Up/Down, Shift for range)");
            #[cfg(test)]
            {
                self.test_keyboard = Some(keyboard_button.clone());
            }
            if keyboard_button.clicked() {
                self.table_focus = true;
                if !keyboard_button.has_focus() {
                    keyboard_button.request_focus();
                }
            }
            if keyboard_button.has_focus() {
                ctx.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        keyboard_button.id,
                        egui::EventFilter {
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    )
                });
            }
            if self.table_focus && keyboard_button.has_focus() {
                let direction = ctx.input(|input| {
                    if input.key_pressed(egui::Key::ArrowDown) {
                        Some(true)
                    } else if input.key_pressed(egui::Key::ArrowUp) {
                        Some(false)
                    } else {
                        None
                    }
                });
                if let Some(forward) = direction {
                    // The first key can arrive before egui has installed the focus filter.
                    ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                    self.selection.navigate(
                        &self.rows,
                        forward,
                        ctx.input(|input| input.modifiers.shift),
                    );
                    self.scroll_to = self
                        .selection
                        .focus
                        .and_then(|id| self.rows.iter().position(|row| *row == ViewRow::Track(id)));
                }
            }
            let layout = if self.rtl {
                egui::Layout::right_to_left(egui::Align::Center)
            } else {
                egui::Layout::left_to_right(egui::Align::Center)
            };
            ui.allocate_ui_with_layout(
                Vec2::new(
                    self.widths.iter().sum::<f32>() + 2.0 * ui.spacing().item_spacing.x,
                    ROW_HEIGHT,
                ),
                layout,
                |ui| {
                    for slot in 0..self.columns.len() {
                        let column = self.columns[slot];
                        let title = match column {
                            Column::Title => "Title",
                            Column::Album => "Album",
                            Column::Identity => "Record ID",
                        };
                        let response = ui.add_sized(
                            [self.widths[slot], ROW_HEIGHT],
                            egui::Label::new(RichText::new(title).strong())
                                .sense(Sense::click_and_drag()),
                        );
                        response.dnd_set_drag_payload(ColumnDrag(column));
                        if let Some(source) = response.dnd_release_payload::<ColumnDrag>()
                            && let Some(from) =
                                self.columns.iter().position(|column| *column == source.0)
                        {
                            self.columns.swap(from, slot);
                            self.widths.swap(from, slot);
                        }
                    }
                },
            );
            let mut clicked = None;
            let mut moved = None;
            #[cfg(not(target_arch = "wasm32"))]
            let mut missing_art = Vec::with_capacity(32);
            let rows = self.rows.clone(); // Arc only; never clone the catalog/view per frame.
            let mut scroll = egui::ScrollArea::both().id_salt("library");
            if let Some(index) = self.scroll_to.take() {
                scroll = scroll.vertical_scroll_offset(
                    index as f32 * (ROW_HEIGHT + ui.spacing().item_spacing.y),
                );
            }
            scroll.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, visible| {
                self.rows_drawn = visible.len();
                for index in visible {
                    let row = rows[index];
                    let row_response = ui
                        .push_id(index, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(
                                    self.widths.iter().sum::<f32>()
                                        + 2.0 * ui.spacing().item_spacing.x,
                                    ROW_HEIGHT,
                                ),
                                layout,
                                |ui| match row {
                                    ViewRow::Album(album) => {
                                        ui.add_sized(
                                            [self.widths.iter().sum(), ROW_HEIGHT],
                                            egui::Label::new(
                                                RichText::new(format!("Album {album}")).strong(),
                                            ),
                                        );
                                    }
                                    ViewRow::Track(id) => {
                                        let record = &self.data[id];
                                        for slot in 0..3 {
                                            let text = match self.columns[slot] {
                                                Column::Title => record.title.clone(),
                                                Column::Album => format!("Album {}", record.album),
                                                Column::Identity => id.to_string(),
                                            };
                                            let selected = self.selection.selected.contains(&id);
                                            let button =
                                                egui::Button::selectable(selected, text.as_str());
                                            #[cfg(not(target_arch = "wasm32"))]
                                            let button = if self.artwork_enabled
                                                && self.columns[slot] == Column::Title
                                            {
                                                if let Some(texture) = self.artwork.texture(id) {
                                                    egui::Button::image_and_text(
                                                        (texture, Vec2::splat(16.0)),
                                                        text.as_str(),
                                                    )
                                                    .selected(selected)
                                                } else {
                                                    if missing_art.len() < 32 {
                                                        missing_art.push(id);
                                                    }
                                                    button
                                                }
                                            } else {
                                                button
                                            };
                                            let response = ui.add_sized(
                                                [self.widths[slot], ROW_HEIGHT],
                                                button.sense(Sense::click_and_drag()),
                                            );
                                            if response.clicked() {
                                                clicked = Some(id);
                                            }
                                            response.dnd_set_drag_payload(TrackDrag(id));
                                            if let Some(source) =
                                                response.dnd_release_payload::<TrackDrag>()
                                            {
                                                moved = Some((source.0, id));
                                            }
                                            response.context_menu(|ui| {
                                                if ui.button("Copy synthetic title").clicked() {
                                                    ctx.copy_text(record.title.clone());
                                                    ui.close();
                                                }
                                            });
                                        }
                                    }
                                },
                            );
                        })
                        .response;
                    self.max_row_height = self.max_row_height.max(row_response.rect.height());
                    if self.scroll_to == Some(index) {
                        row_response.scroll_to_me(Some(egui::Align::Center));
                        self.scroll_to = None;
                    }
                }
            });
            #[cfg(not(target_arch = "wasm32"))]
            self.artwork.request(&missing_art);
            if let Some(id) = clicked {
                self.table_focus = true;
                if !keyboard_button.has_focus() {
                    keyboard_button.request_focus();
                }
                let modifiers = ctx.input(|input| input.modifiers);
                self.selection
                    .select(&rows, id, modifiers.shift, modifiers.command);
            }
            if let Some(pair) = moved
                && !self.query.grouped
            {
                self.query.move_before = Some(pair);
                self.submit(ctx);
            }
        });
        // No periodic repaint for a quiescent table. PCM producers request repaint.
        if self.pending {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

impl eframe::App for SpikeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

/// Original spectrum/waveform preset in the style of upstream audioMotion, not a preset port.
pub fn paint_preset(painter: &egui::Painter, rect: egui::Rect, frame: &Analysis) {
    let width = rect.width() / frame.bands.len() as f32;
    for (index, value) in frame.bands.iter().enumerate() {
        let bar = egui::Rect::from_min_max(
            egui::pos2(
                rect.left() + index as f32 * width,
                rect.bottom() - value * rect.height(),
            ),
            egui::pos2(
                rect.left() + (index + 1) as f32 * width - 2.0,
                rect.bottom(),
            ),
        );
        painter.rect_filled(bar, 1.0, Color32::from_rgb(90, 190, 160));
    }
    let points: Vec<_> = frame
        .waveform
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            egui::pos2(
                rect.left() + index as f32 * rect.width() / frame.waveform.len() as f32,
                rect.center().y - sample * rect.height() * 0.4,
            )
        })
        .collect();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.0, Color32::LIGHT_BLUE),
    ));
}

pub fn analysis_from_pcm(pcm: &[f32]) -> Option<Analysis> {
    Analysis::from_pcm(pcm)
}

#[cfg(test)]
#[path = "../../../tests/spike-ui/keyboard.rs"]
mod keyboard_tests;
