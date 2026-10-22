//! Bounded owned-PNG decode/load experiment, not network artwork or a server cache.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
const CAPACITY: usize = 64;
const BATCH: usize = 32;
type Requests = Arc<(Mutex<Option<Vec<usize>>>, Condvar)>;
type Results = Arc<Mutex<Vec<(usize, egui::ColorImage)>>>;

pub(super) struct Artwork {
    cache: BTreeMap<usize, (egui::TextureHandle, u64)>,
    revision: u64,
    request: Requests,
    result: Results,
    stopped: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Artwork {
    pub(super) fn new(ctx: egui::Context) -> Self {
        let request: Requests = Arc::new((Mutex::new(None), Condvar::new()));
        let result: Results = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let (requests, results, stop) = (request.clone(), result.clone(), stopped.clone());
        let worker = std::thread::spawn(move || {
            loop {
                let mut slot = requests
                    .0
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                while slot.is_none() && !stop.load(Ordering::Acquire) {
                    slot = requests
                        .1
                        .wait(slot)
                        .unwrap_or_else(|poison| poison.into_inner());
                }
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let batch = slot.take().unwrap_or_default();
                drop(slot);
                let mut decoded = Vec::with_capacity(BATCH);
                for id in batch.into_iter().take(BATCH) {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    // Only this fixed 32x32 owned fixture is decoded; no unbounded external image input.
                    let image = image::load_from_memory(include_bytes!(
                        "../../../tools/app/assets/p01-thumbnail.png"
                    ))
                    .expect("embedded 32x32 PNG is verified by the artwork fixture regression");
                    let rgba = image.into_rgba8();
                    decoded.push((
                        id,
                        egui::ColorImage::from_rgba_unmultiplied([32, 32], rgba.as_raw()),
                    ));
                }
                *results.lock().unwrap_or_else(|poison| poison.into_inner()) = decoded;
                ctx.request_repaint();
            }
        });
        Self {
            cache: BTreeMap::new(),
            revision: 0,
            request,
            result,
            stopped,
            worker: Some(worker),
        }
    }
    pub(super) fn receive(&mut self, ctx: &egui::Context) {
        let images = std::mem::take(
            &mut *self
                .result
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        );
        for (id, image) in images {
            if self.cache.contains_key(&id) {
                continue;
            }
            if self.cache.len() >= CAPACITY
                && let Some(oldest) = self
                    .cache
                    .iter()
                    .min_by_key(|(_, (_, revision))| *revision)
                    .map(|(id, _)| *id)
            {
                self.cache.remove(&oldest);
            }
            self.revision += 1;
            self.cache.insert(
                id,
                (
                    ctx.load_texture(
                        format!("owned-art-{id}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                    self.revision,
                ),
            );
        }
    }
    pub(super) fn texture(&mut self, id: usize) -> Option<egui::TextureId> {
        let (handle, revision) = self.cache.get_mut(&id)?;
        self.revision += 1;
        *revision = self.revision;
        Some(handle.id())
    }
    pub(super) fn request(&self, ids: &[usize]) {
        if ids.is_empty() {
            return;
        }
        *self
            .request
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) =
            Some(ids.iter().copied().take(BATCH).collect());
        self.request.1.notify_one();
    }
}
impl Drop for Artwork {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let guard = self
            .request
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.request.1.notify_one();
        drop(guard);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
#[path = "../../../tests/spike-ui/artwork.rs"]
mod tests;
