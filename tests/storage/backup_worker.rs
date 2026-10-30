#![cfg(not(target_arch = "wasm32"))]
use feishin_domain::*;
use feishin_storage::{
    AtomicStore, NativePlaylistBackups, NativeStore, playlist_backups::Repository,
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};
#[derive(Default)]
struct State {
    now: Cell<u64>,
    cancelled: Cell<bool>,
}
impl Clock for State {
    fn now(&self) -> Milliseconds {
        Milliseconds(self.now.get())
    }
}
impl Cancellation for State {
    fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }
}
fn context(state: &State) -> OperationContext<'_> {
    OperationContext {
        correlation: RequestContext {
            request: RequestId(7),
            generation: Generation(9),
            deadline: Milliseconds(10000),
        },
        limits: OperationLimits {
            items: 10000,
            request_bytes: 256 * 1024,
            response_bytes: 256 * 1024,
        },
        cancellation: state,
    }
}
fn backup(server: &str) -> PlaylistBackup {
    PlaylistBackup {
        playlist: Scoped {
            server: ServerId::new(server),
            id: PlaylistId::new("p"),
        },
        name: "Saved".into(),
        songs: vec![
            SongId::new("same"),
            SongId::new("other"),
            SongId::new("same"),
        ],
        captured_at: UnixMilliseconds(1234567890123),
    }
}
struct WakeThread(thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn poll<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(WakeThread(thread::current())));
    future.poll(&mut Context::from_waker(&waker))
}
fn run<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(value) = poll(future.as_mut()) {
            return value;
        }
        assert!(Instant::now() < end, "worker did not wake/finish");
        thread::park_timeout(Duration::from_millis(20));
    }
}
fn close(worker: &mut NativePlaylistBackups) -> Result<(), BackupFailure> {
    worker.begin_shutdown();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = worker.try_join() {
            return result;
        }
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn native_worker_reopen_scopes_duplicates_and_exact_receipts() {
    let root = tempfile::tempdir().unwrap();
    let state = State::default();
    let mut worker = NativePlaylistBackups::open(root.path().to_owned()).unwrap();
    let a = run(worker.save(backup("a"), &state, context(&state))).unwrap();
    assert_eq!(a.context, context(&state).correlation);
    assert_eq!(a.revision, Revision(1));
    let mut next = context(&state);
    next.correlation.request = RequestId(8);
    assert_eq!(
        run(worker.save(backup("b"), &state, next))
            .unwrap()
            .revision,
        Revision(2)
    );
    close(&mut worker).unwrap();
    let mut repository = Repository::new(NativeStore::open_playlist_backups(root.path()).unwrap());
    let s = repository.load().unwrap();
    assert_eq!(s.backups.len(), 2);
    assert_eq!(s.backups[0].song_ids, ["same", "other", "same"]);
    assert_eq!(s.backups[0].captured_at_unix_ms, 1234567890123);
    assert_eq!(
        run(worker.save(backup("a"), &state, context(&state)))
            .unwrap_err()
            .cause,
        BackupError::Unavailable
    );
}
struct Memory {
    bytes: Rc<RefCell<Option<Vec<u8>>>>,
    writes: Arc<AtomicUsize>,
    main: thread::ThreadId,
}
impl AtomicStore for Memory {
    fn read(&mut self) -> Result<Option<Vec<u8>>, feishin_storage::Error> {
        assert_ne!(thread::current().id(), self.main);
        Ok(self.bytes.borrow().clone())
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), feishin_storage::Error> {
        assert_ne!(thread::current().id(), self.main);
        *self.bytes.borrow_mut() = Some(bytes.to_vec());
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn non_send_store_constructed_on_worker_and_preflight_has_no_io() {
    let state = State::default();
    let main = thread::current().id();
    let calls = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let called = Arc::clone(&calls);
    let written = Arc::clone(&writes);
    let mut worker = NativePlaylistBackups::with_factory(move || {
        called.fetch_add(1, Ordering::SeqCst);
        assert_ne!(thread::current().id(), main);
        Ok(Memory {
            bytes: Rc::default(),
            writes: written,
            main,
        })
    })
    .unwrap();
    state.cancelled.set(true);
    assert_eq!(
        run(worker.save(backup("s"), &state, context(&state))).unwrap_err(),
        BackupFailure {
            cause: BackupError::Context(DomainError::Cancelled),
            may_have_written: false
        }
    );
    state.cancelled.set(false);
    state.now.set(10001);
    assert_eq!(
        run(worker.save(backup("s"), &state, context(&state)))
            .unwrap_err()
            .cause,
        BackupError::Context(DomainError::Deadline)
    );
    state.now.set(0);
    let mut invalid = backup("s");
    invalid.captured_at = UnixMilliseconds(u64::MAX);
    assert_eq!(
        run(worker.save(invalid, &state, context(&state)))
            .unwrap_err()
            .cause,
        BackupError::Invalid
    );
    let mut large = backup("s");
    large.name = "a".repeat(256 * 1024);
    assert_eq!(
        run(worker.save(large, &state, context(&state)))
            .unwrap_err()
            .cause,
        BackupError::Capacity
    );
    let mut limited = context(&state);
    limited.limits.items = 2;
    assert_eq!(
        run(worker.save(backup("s"), &state, limited))
            .unwrap_err()
            .cause,
        BackupError::Context(DomainError::Capacity)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    run(worker.save(backup("s"), &state, context(&state))).unwrap();
    close(&mut worker).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}
struct Blocked {
    started: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
    writes: Arc<AtomicUsize>,
    in_write: bool,
    fail: bool,
}
impl AtomicStore for Blocked {
    fn read(&mut self) -> Result<Option<Vec<u8>>, feishin_storage::Error> {
        if !self.in_write {
            self.started.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        Ok(None)
    }
    fn replace(&mut self, _: &[u8]) -> Result<(), feishin_storage::Error> {
        if self.in_write {
            self.started.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(feishin_storage::Error::Io)
        } else {
            Ok(())
        }
    }
}
fn blocked(
    in_write: bool,
    fail: bool,
) -> (
    NativePlaylistBackups,
    mpsc::Receiver<()>,
    mpsc::SyncSender<()>,
    Arc<AtomicUsize>,
) {
    let (started, ready) = mpsc::sync_channel(1);
    let (go, release) = mpsc::sync_channel(1);
    let writes = Arc::new(AtomicUsize::new(0));
    let written = Arc::clone(&writes);
    let worker = NativePlaylistBackups::with_factory(move || {
        Ok(Blocked {
            started,
            release,
            writes: written,
            in_write,
            fail,
        })
    })
    .unwrap();
    (worker, ready, go, writes)
}
#[test]
fn dropped_future_cancels_before_write_and_keeps_admission_bounded() {
    let state = State::default();
    let (mut worker, ready, go, writes) = blocked(false, false);
    let mut future = worker.save(backup("s"), &state, context(&state));
    assert!(poll(future.as_mut()).is_pending());
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(future);
    assert!(!worker.is_idle());
    assert_eq!(
        run(worker.save(backup("other"), &state, context(&state)))
            .unwrap_err()
            .cause,
        BackupError::Capacity
    );
    go.send(()).unwrap();
    close(&mut worker).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 0);
}
#[test]
fn pending_deadline_returns_on_poll_without_blocking_or_claiming_no_write() {
    let state = State::default();
    let (mut worker, ready, go, writes) = blocked(true, false);
    let mut future = worker.save(backup("s"), &state, context(&state));
    assert!(poll(future.as_mut()).is_pending());
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    state.now.set(10001);
    assert_eq!(
        poll(future.as_mut()),
        Poll::Ready(Err(BackupFailure {
            cause: BackupError::Context(DomainError::Deadline),
            may_have_written: true
        }))
    );
    drop(future);
    assert!(!worker.is_idle());
    go.send(()).unwrap();
    close(&mut worker).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}
#[test]
fn uncertain_io_factory_failure_and_panic_are_terminal_and_wake_waiters() {
    let state = State::default();
    let (mut worker, ready, go, writes) = blocked(true, true);
    let mut future = worker.save(backup("s"), &state, context(&state));
    assert!(poll(future.as_mut()).is_pending());
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    go.send(()).unwrap();
    let err = run(future).unwrap_err();
    assert_eq!(
        err,
        BackupFailure {
            cause: BackupError::Unavailable,
            may_have_written: true
        }
    );
    close(&mut worker).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    let mut worker = NativePlaylistBackups::with_factory::<NativeStore, _>(|| {
        Err(feishin_storage::Error::UnsafePath)
    })
    .unwrap();
    assert_eq!(
        run(worker.save(backup("s"), &state, context(&state))).unwrap_err(),
        BackupFailure {
            cause: BackupError::Unavailable,
            may_have_written: false
        }
    );
    close(&mut worker).unwrap();
    let mut worker = NativePlaylistBackups::with_factory::<NativeStore, _>(|| {
        panic!("owned test factory panic")
    })
    .unwrap();
    assert_eq!(
        run(worker.save(backup("s"), &state, context(&state))).unwrap_err(),
        BackupFailure {
            cause: BackupError::Unavailable,
            may_have_written: true
        }
    );
    assert!(close(&mut worker).is_err());
}

#[test]
fn escaped_write_budget_rejects_without_creating_a_value() {
    let root = tempfile::tempdir().unwrap();
    let state = State::default();
    let mut worker = NativePlaylistBackups::open(root.path().to_owned()).unwrap();
    let mut value = backup("s");
    value.name = "\n".repeat(100);
    let mut limited = context(&state);
    limited.limits.request_bytes = 200;
    assert_eq!(
        run(worker.save(value, &state, limited)).unwrap_err(),
        BackupFailure {
            cause: BackupError::Capacity,
            may_have_written: false
        }
    );
    close(&mut worker).unwrap();
    assert!(
        !NativeStore::open_playlist_backups(root.path())
            .unwrap()
            .path()
            .exists()
    );
}
#[test]
fn factory_wait_obeys_latest_start_and_completion_really_wakes() {
    let state = State::default();
    let writes = Arc::new(AtomicUsize::new(0));
    let written = Arc::clone(&writes);
    let main = thread::current().id();
    let (started, ready) = mpsc::sync_channel(1);
    let (go, release) = mpsc::sync_channel(1);
    let mut worker = NativePlaylistBackups::with_factory(move || {
        started.send(()).unwrap();
        release.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(Memory {
            bytes: Rc::default(),
            writes: written,
            main,
        })
    })
    .unwrap();
    let mut limited = context(&state);
    limited.correlation.deadline = Milliseconds(1000);
    let mut future = worker.save(backup("s"), &state, limited);
    let (notice, woke) = mpsc::sync_channel(1);
    struct Notice(mpsc::SyncSender<()>);
    impl Wake for Notice {
        fn wake(self: Arc<Self>) {
            self.0.send(()).unwrap();
        }
    }
    let waker = Waker::from(Arc::new(Notice(notice)));
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    thread::sleep(Duration::from_millis(1050));
    go.send(()).unwrap();
    woke.recv_timeout(Duration::from_secs(2))
        .expect("worker must wake the pending future");
    assert_eq!(
        run(future).unwrap_err(),
        BackupFailure {
            cause: BackupError::Context(DomainError::Deadline),
            may_have_written: false
        }
    );
    close(&mut worker).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 0);
}
#[test]
fn drop_does_not_join_blocked_filesystem_work() {
    let state = State::default();
    let (mut worker, ready, go, _) = blocked(true, false);
    let mut future = worker.save(backup("s"), &state, context(&state));
    assert!(poll(future.as_mut()).is_pending());
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(future);
    let (notice, dropped) = mpsc::sync_channel(1);
    let dropper = thread::spawn(move || {
        drop(worker);
        notice.send(()).unwrap();
    });
    let result = dropped.recv_timeout(Duration::from_secs(2));
    go.send(()).unwrap(); // Release the owned OS double even if Drop incorrectly waited.
    result.unwrap();
    dropper.join().unwrap();
}

struct SecondRead {
    reads: usize,
    started: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
    writes: Arc<AtomicUsize>,
}
impl AtomicStore for SecondRead {
    fn read(&mut self) -> Result<Option<Vec<u8>>, feishin_storage::Error> {
        self.reads += 1;
        if self.reads == 2 {
            self.started.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        Ok(None)
    }
    fn replace(&mut self, _: &[u8]) -> Result<(), feishin_storage::Error> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn cancellation_and_latest_start_during_final_read_prevent_replacement() {
    for cancel in [true, false] {
        let state = State::default();
        let writes = Arc::new(AtomicUsize::new(0));
        let written = Arc::clone(&writes);
        let (started, ready) = mpsc::sync_channel(1);
        let (go, release) = mpsc::sync_channel(1);
        let mut worker = NativePlaylistBackups::with_factory(move || {
            Ok(SecondRead {
                reads: 0,
                started,
                release,
                writes: written,
            })
        })
        .unwrap();
        let mut limited = context(&state);
        if !cancel {
            limited.correlation.deadline = Milliseconds(1000);
        }
        let mut future = worker.save(backup("s"), &state, limited);
        assert!(poll(future.as_mut()).is_pending());
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        if cancel {
            drop(future);
            go.send(()).unwrap();
        } else {
            thread::sleep(Duration::from_millis(1050));
            go.send(()).unwrap();
            assert_eq!(
                run(future).unwrap_err(),
                BackupFailure {
                    cause: BackupError::Context(DomainError::Deadline),
                    may_have_written: false
                }
            );
        }
        close(&mut worker).unwrap();
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }
}
