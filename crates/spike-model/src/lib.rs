//! Disposable P01 feasibility data, not the application's domain or queue model.
use std::{collections::BTreeSet, sync::Arc};

pub const MAX_RECORDS: usize = 100_000;
pub const PCM_SAMPLES: usize = 256;
pub const BANDS: usize = 16;
pub const BAND_BINS: [usize; BANDS] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 80, 96, 112, 127];

#[derive(Clone, Debug)]
pub struct Record {
    pub id: usize,
    pub title: String,
    pub album: usize,
}

pub fn records(count: usize) -> Arc<Vec<Record>> {
    Arc::new(
        (0..count.min(MAX_RECORDS))
            .map(|id| Record {
                id,
                title: format!("Track {id:06}"),
                album: id / 10,
            })
            .collect(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRow {
    Album(usize),
    Track(usize),
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,
    pub descending: bool,
    pub grouped: bool,
    /// A single prototype drag operation, not a durable library mutation.
    pub move_before: Option<(usize, usize)>,
}

/// Called on a bounded worker on native targets. Aborted results must not be published.
pub fn build_view(
    records: &[Record],
    query: &Query,
    cancelled: impl Fn() -> bool,
) -> Option<Vec<ViewRow>> {
    let text = query.text.to_lowercase();
    let mut ids = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        if record.title.to_lowercase().contains(&text) {
            ids.push(index);
        }
    }
    // Sorting is worker work. Group keys remain contiguous even for imported test metadata.
    ids.sort_unstable_by(|left, right| {
        let left_record = &records[*left];
        let right_record = &records[*right];
        let group = if query.grouped {
            left_record.album.cmp(&right_record.album)
        } else {
            std::cmp::Ordering::Equal
        };
        group
            .then_with(|| left_record.title.cmp(&right_record.title))
            .then_with(|| left.cmp(right))
    });
    if cancelled() {
        return None;
    }
    if query.descending {
        ids.reverse();
    }
    if !query.grouped
        && let Some((from, to)) = query.move_before
        && let (Some(source), Some(_)) = (
            ids.iter().position(|id| *id == from),
            ids.iter().position(|id| *id == to),
        )
        && from != to
    {
        let id = ids.remove(source);
        if let Some(target) = ids.iter().position(|id| *id == to) {
            ids.insert(target, id);
        }
    }
    let mut rows =
        Vec::with_capacity(ids.len() + if query.grouped { ids.len() / 10 + 1 } else { 0 });
    let mut album = None;
    for (position, id) in ids.into_iter().enumerate() {
        if position % 256 == 0 && cancelled() {
            return None;
        }
        let record = &records[id]; // IDs come exclusively from records() and its unchanged vector.
        if query.grouped && album != Some(record.album) {
            album = Some(record.album);
            rows.push(ViewRow::Album(record.album));
        }
        rows.push(ViewRow::Track(id));
    }
    (!cancelled()).then_some(rows)
}

#[derive(Default)]
pub struct Selection {
    pub selected: BTreeSet<usize>,
    pub focus: Option<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn select(&mut self, view: &[ViewRow], id: usize, extend: bool, toggle: bool) {
        let Some(target) = view.iter().position(|row| *row == ViewRow::Track(id)) else {
            return;
        };
        if extend {
            let anchor = self
                .anchor
                .and_then(|id| view.iter().position(|row| *row == ViewRow::Track(id)))
                .unwrap_or(target);
            if !toggle {
                self.selected.clear();
            }
            for row in &view[anchor.min(target)..=anchor.max(target)] {
                if let ViewRow::Track(id) = row {
                    self.selected.insert(*id);
                }
            }
        } else {
            if !toggle {
                self.selected.clear();
            }
            if !toggle || !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.anchor = Some(id);
        }
        self.focus = Some(id);
    }

    pub fn navigate(&mut self, view: &[ViewRow], forward: bool, extend: bool) -> Option<usize> {
        let current = self
            .focus
            .and_then(|id| view.iter().position(|row| *row == ViewRow::Track(id)));
        let target = match current {
            Some(index) if forward => {
                (index + 1..view.len()).find(|index| matches!(view[*index], ViewRow::Track(_)))
            }
            Some(index) => (0..index)
                .rev()
                .find(|index| matches!(view[*index], ViewRow::Track(_))),
            None if forward => view.iter().position(|row| matches!(row, ViewRow::Track(_))),
            None => view
                .iter()
                .rposition(|row| matches!(row, ViewRow::Track(_))),
        };
        if let Some(index) = target
            && let ViewRow::Track(id) = view[index]
        {
            self.select(view, id, extend, false);
        }
        self.focus
    }
}

/// Small, bounded analysis frame. Non-finite/device data cannot poison the renderer.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub waveform: [f32; PCM_SAMPLES],
    pub bands: [f32; BANDS],
}

impl Analysis {
    pub fn from_pcm(pcm: &[f32]) -> Option<Self> {
        if pcm.len() != PCM_SAMPLES || pcm.iter().any(|sample| !sample.is_finite()) {
            return None;
        }
        let mut frame = Self {
            waveform: [0.0; PCM_SAMPLES],
            bands: [0.0; BANDS],
        };
        for (out, input) in frame.waveform.iter_mut().zip(pcm) {
            *out = input.clamp(-1.0, 1.0);
        }
        for (index, band) in frame.bands.iter_mut().enumerate() {
            let mut real = 0.0;
            let mut imag = 0.0;
            for (sample, value) in frame.waveform.iter().enumerate() {
                let phase = std::f32::consts::TAU * BAND_BINS[index] as f32 * sample as f32
                    / PCM_SAMPLES as f32;
                real += value * phase.cos();
                imag -= value * phase.sin();
            }
            *band = (2.0 * real.hypot(imag) / PCM_SAMPLES as f32).min(1.0);
        }
        Some(frame)
    }
}

/// Keep CSS inert. This report is not a settings importer or a CSS parser.
pub fn css_report(css: &str) -> &'static str {
    if css.trim().is_empty() {
        "No CSS supplied"
    } else {
        "CSS retained as inert text for this session. Native replacements: theme, scale and column widths. Other rules are unsupported; no styles executed."
    }
}

#[cfg(test)]
#[path = "../../../tests/spike-model/private.rs"]
mod tests;
