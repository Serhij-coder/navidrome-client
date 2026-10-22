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

#[derive(Clone, Debug)]
pub enum DragSource {
    Tracks(Arc<BTreeSet<usize>>),
    Album(usize),
}

/// A session-only view operation. Does not change records, album membership or a queue.
#[derive(Clone, Debug)]
pub struct Reorder {
    pub rows: Arc<Vec<ViewRow>>,
    pub source: DragSource,
    pub before: ViewRow,
    pub grouped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderError {
    ChangedView,
    CrossAlbum,
}

impl ReorderError {
    pub fn message(self) -> &'static str {
        match self {
            Self::ChangedView => "The view changed. Try dragging again.",
            Self::CrossAlbum => {
                "Tracks can move within one album. Drag an album header to move its whole group."
            }
        }
    }
}

/// Bounded, cancellable worker operation preserving displayed order and group identity.
/// Selection outside the filtered view is not moved. A grouped track move across
/// albums is rejected, rather than silently editing metadata or splitting groups.
pub fn reorder_view(
    records: &[Record],
    request: &Reorder,
    cancelled: impl Fn() -> bool,
) -> Option<Result<Vec<ViewRow>, ReorderError>> {
    use ReorderError::{ChangedView, CrossAlbum};
    let rows = &request.rows;
    let mut seen = vec![false; records.len()];
    let mut albums = BTreeSet::new();
    let mut album = None;
    let mut target_album = None;
    let mut source_album = None;
    let mut source_found = false;
    let mut cross_album = false;
    let mut target_found = false;
    for (index, row) in rows.iter().enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        match *row {
            ViewRow::Album(id) => {
                if !request.grouped || !albums.insert(id) {
                    return Some(Err(ChangedView));
                }
                album = Some(id);
                if matches!(request.source, DragSource::Album(source) if source == id) {
                    source_found = true;
                }
            }
            ViewRow::Track(id) => {
                let Some(record) = records.get(id) else {
                    return Some(Err(ChangedView));
                };
                if seen[id] || (request.grouped && album != Some(record.album)) {
                    return Some(Err(ChangedView));
                }
                seen[id] = true;
                if let DragSource::Tracks(ids) = &request.source
                    && ids.contains(&id)
                {
                    source_found = true;
                    if source_album.is_some_and(|source| source != record.album) {
                        cross_album = true;
                    }
                    source_album = Some(record.album);
                }
            }
        }
        if *row == request.before {
            target_found = true;
            target_album = match *row {
                ViewRow::Album(id) => Some(id),
                ViewRow::Track(id) => Some(records[id].album),
            };
        }
    }
    if !source_found || !target_found {
        return Some(Err(ChangedView));
    }
    if request.grouped
        && matches!(request.source, DragSource::Tracks(_))
        && (cross_album || source_album != target_album)
    {
        return Some(Err(CrossAlbum));
    }
    let mut moved = Vec::new();
    let mut rest = Vec::with_capacity(rows.len());
    album = None;
    for (index, row) in rows.iter().enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        if let ViewRow::Album(id) = row {
            album = Some(*id);
        }
        let selected = match &request.source {
            DragSource::Tracks(ids) => matches!(row, ViewRow::Track(id) if ids.contains(id)),
            DragSource::Album(id) => album == Some(*id),
        };
        if selected {
            moved.push(*row);
        } else {
            rest.push(*row);
        }
    }
    // Dropping onto the dragged block is a no-op, including its own album header.
    let destination = match request.source {
        DragSource::Album(_) => ViewRow::Album(target_album?),
        DragSource::Tracks(_) => request.before,
    };
    let mut output = Vec::with_capacity(rows.len());
    if let Some(index) = rest.iter().position(|row| *row == destination) {
        // Track drops onto their album header mean the start of that album.
        let index = index
            + usize::from(
                matches!(request.source, DragSource::Tracks(_))
                    && matches!(destination, ViewRow::Album(_)),
            );
        output.extend_from_slice(&rest[..index]);
        output.append(&mut moved);
        output.extend_from_slice(&rest[index..]);
    } else {
        output.extend_from_slice(rows);
    }
    (!cancelled()).then_some(Ok(output))
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,
    pub descending: bool,
    pub grouped: bool,
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
    pub selected: Arc<BTreeSet<usize>>,
    pub focus: Option<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn select(&mut self, view: &[ViewRow], id: usize, extend: bool, toggle: bool) {
        let Some(target) = view.iter().position(|row| *row == ViewRow::Track(id)) else {
            return;
        };
        let selected = Arc::make_mut(&mut self.selected);
        if extend {
            let anchor = self
                .anchor
                .and_then(|id| view.iter().position(|row| *row == ViewRow::Track(id)))
                .unwrap_or(target);
            if !toggle {
                selected.clear();
            }
            for row in &view[anchor.min(target)..=anchor.max(target)] {
                if let ViewRow::Track(id) = row {
                    selected.insert(*id);
                }
            }
        } else {
            if !toggle {
                selected.clear();
            }
            if !toggle || !selected.remove(&id) {
                selected.insert(id);
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
