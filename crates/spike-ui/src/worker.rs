use feishin_spike_model::{Query, Record, ViewRow, build_view};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

type RequestSlot = Arc<(Mutex<Option<(u64, Query)>>, Condvar)>;
type ResultSlot = Arc<Mutex<Option<(u64, Vec<ViewRow>)>>>;

pub(super) struct Worker {
    request: RequestSlot,
    result: ResultSlot,
    generation: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub(super) fn new(data: Arc<Vec<Record>>, ctx: egui::Context) -> Self {
        let request: RequestSlot = Arc::new((Mutex::new(None), Condvar::new()));
        let result: ResultSlot = Arc::new(Mutex::new(None));
        let generation = Arc::new(AtomicU64::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let (requests, results, current, stop) = (
            request.clone(),
            result.clone(),
            generation.clone(),
            stopped.clone(),
        );
        let thread = std::thread::spawn(move || {
            loop {
                let (lock, wake) = &*requests;
                let mut slot = lock.lock().unwrap_or_else(|poison| poison.into_inner());
                while slot.is_none() && !stop.load(Ordering::Acquire) {
                    slot = wake.wait(slot).unwrap_or_else(|poison| poison.into_inner());
                }
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let Some((id, query)) = slot.take() else {
                    continue;
                };
                drop(slot);
                let cancelled =
                    || stop.load(Ordering::Acquire) || current.load(Ordering::Acquire) != id;
                if let Some(rows) = build_view(&data, &query, cancelled)
                    && !cancelled()
                {
                    *results.lock().unwrap_or_else(|poison| poison.into_inner()) = Some((id, rows));
                    ctx.request_repaint();
                }
            }
        });
        Self {
            request,
            result,
            generation,
            stopped,
            thread: Some(thread),
        }
    }
    pub(super) fn submit(&self, id: u64, query: Query) {
        self.generation.store(id, Ordering::Release);
        *self
            .request
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some((id, query));
        self.request.1.notify_one();
    }
    pub(super) fn take(&self) -> Option<(u64, Vec<ViewRow>)> {
        self.result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // Acquire the predicate lock before notifying to avoid a lost shutdown wakeup.
        let guard = self
            .request
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.request.1.notify_one();
        drop(guard);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/spike-ui/worker.rs"]
mod tests;
