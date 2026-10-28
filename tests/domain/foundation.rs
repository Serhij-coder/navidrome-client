use feishin_domain::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

fn occurrence(id: &str, server: &str) -> QueueOccurrence {
    QueueOccurrence {
        id: QueueOccurrenceId::new(id),
        song: Scoped {
            server: ServerId::new(server),
            id: SongId::new("same"),
        },
    }
}
#[test]
fn identity_and_stale_completions() {
    let mut owner = Owner::new(3);
    let Effect::Load { context: old } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(20),
        })
        .unwrap()
        .unwrap();
    let Effect::Load { context: current } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(20),
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        owner.event(
            Event::Loaded {
                context: old,
                items: vec![]
            },
            Milliseconds(1)
        ),
        Err(DomainError::Stale)
    );
    owner
        .event(
            Event::Loaded {
                context: current,
                items: vec![
                    occurrence("1", "a"),
                    occurrence("2", "a"),
                    occurrence("3", "b"),
                ],
            },
            Milliseconds(1),
        )
        .unwrap();
    assert_eq!(owner.items()[0].song, owner.items()[1].song);
    assert_ne!(owner.items()[0].id, owner.items()[1].id);
    assert_ne!(owner.items()[0].song, owner.items()[2].song);
    assert_eq!(owner.revision(), Revision(1));
    assert_eq!(
        owner.command(Command::Add(occurrence("4", "a"))),
        Err(DomainError::Capacity)
    );
}
#[test]
fn conversions_and_capability_membership() {
    assert_eq!(Milliseconds::from_seconds(1.2345), Ok(Milliseconds(1234)));
    assert_eq!(Milliseconds::from_seconds(-1.0), Err(UnitError::Negative));
    assert_eq!(
        Milliseconds::from_seconds(f64::NAN),
        Err(UnitError::NonFinite)
    );
    assert_eq!(Milliseconds(u64::MAX).to_ticks(), Err(UnitError::Overflow));
    assert_eq!(Ticks(19_999).to_milliseconds(), Milliseconds(1));
    assert_eq!(Ticks::from_seconds(0.000_000_15), Ok(Ticks(1)));
    let mut c = Capabilities::default();
    c.0.insert(ServerFeature::ServerPlayQueue, vec![1, 3]);
    assert!(c.has(ServerFeature::ServerPlayQueue));
    assert!(c.has_version(ServerFeature::ServerPlayQueue, 3));
    assert!(!c.has_version(ServerFeature::ServerPlayQueue, 2));
}
#[test]
fn deadline_duplicate_and_local_mutation_reject_replacement() {
    let mut owner = Owner::new(2);
    let Effect::Load { context } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(2),
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        owner.event(
            Event::Loaded {
                context,
                items: vec![]
            },
            Milliseconds(3)
        ),
        Err(DomainError::Deadline)
    );
    let Effect::Load { context } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(4),
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        owner.event(
            Event::Loaded {
                context,
                items: vec![occurrence("1", "a"), occurrence("1", "b")]
            },
            Milliseconds(4)
        ),
        Err(DomainError::InvalidInput)
    );
    let Effect::Load { context } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(4),
        })
        .unwrap()
        .unwrap();
    owner.command(Command::Add(occurrence("2", "a"))).unwrap();
    assert_eq!(
        owner.event(
            Event::Loaded {
                context,
                items: vec![]
            },
            Milliseconds(4)
        ),
        Err(DomainError::Stale)
    );
    assert_eq!(owner.items().len(), 1);
}

include!("../fixtures/p02/domain/song.rs");
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn ready<T>(mut f: LocalFuture<'_, T>) -> T {
    let waker = Waker::from(Arc::new(Noop));
    match f.as_mut().poll(&mut Context::from_waker(&waker)) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("test double must complete immediately"),
    }
}
#[derive(Default)]
struct LocalServices {
    time: Rc<Cell<u64>>,
    cancelled: Rc<Cell<bool>>,
    durable: Rc<RefCell<Option<Vec<u8>>>>,
    secret: Rc<RefCell<Option<Secret>>>,
    playback: Rc<RefCell<Vec<PlaybackCommand>>>,
    playback_controls_disabled: Rc<Cell<bool>>,
}
impl Clock for LocalServices {
    fn now(&self) -> Milliseconds {
        Milliseconds(self.time.get())
    }
}
impl Random for LocalServices {
    fn next_u64(&mut self) -> u64 {
        self.time.set(self.time.get() + 1);
        self.time.get()
    }
}
impl Cancellation for LocalServices {
    fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }
}
impl LocalServices {
    fn context(&self) -> OperationContext<'_> {
        OperationContext {
            correlation: RequestContext {
                request: RequestId(1),
                generation: Generation(2),
                deadline: Milliseconds(10),
            },
            limits: OperationLimits {
                request_bytes: 4,
                response_bytes: 4,
                items: 1,
            },
            cancellation: self,
        }
    }
}
impl Transport for LocalServices {
    fn request<'a>(
        &'a self,
        request: Vec<u8>,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Vec<u8>, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            c.check_request_size(request.len())?;
            c.check_response_size(request.len())?;
            Ok(request)
        })
    }
}
impl Backend for LocalServices {
    fn song<'a>(
        &'a self,
        id: Scoped<SongId>,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Song, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            let mut song = sample_song();
            song.server = id.server;
            song.id = id.id;
            Ok(song)
        })
    }
    fn songs<'a>(
        &'a self,
        q: SongListQuery,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Page<Song>, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            c.check_items(1)?;
            let mut song = sample_song();
            song.server = q.server;
            Ok(Page {
                items: vec![song],
                start_index: q.start_index,
                total_record_count: None,
            })
        })
    }
    fn capabilities<'a>(
        &'a self,
        _: ServerId,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Capabilities, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            Ok(Capabilities::default())
        })
    }
}
impl Persistence for LocalServices {
    fn load<'a>(
        &'a self,
        _: &'a str,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Option<Vec<u8>>, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            let v = self.durable.borrow().clone();
            c.check_response_size(v.as_ref().map_or(0, Vec::len))?;
            Ok(v)
        })
    }
    fn save<'a>(
        &'a self,
        _: &'a str,
        v: Vec<u8>,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            c.check_request_size(v.len())?;
            *self.durable.borrow_mut() = Some(v);
            Ok(())
        })
    }
    fn remove<'a>(
        &'a self,
        _: &'a str,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            *self.durable.borrow_mut() = None;
            Ok(())
        })
    }
}
impl SecretStore for LocalServices {
    fn get<'a>(
        &'a self,
        _: &'a str,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Option<Secret>, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            let secret = self.secret.borrow();
            if let Some(s) = secret.as_ref() {
                c.check_response_size(s.expose().len())?;
            }
            Ok(secret.as_ref().map(|s| Secret::new(s.expose().to_vec())))
        })
    }
    fn set<'a>(
        &'a self,
        _: &'a str,
        v: Secret,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            c.check_request_size(v.expose().len())?;
            *self.secret.borrow_mut() = Some(v);
            Ok(())
        })
    }
    fn remove<'a>(
        &'a self,
        _: &'a str,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            *self.secret.borrow_mut() = None;
            Ok(())
        })
    }
}
impl Playback for LocalServices {
    fn capabilities(&self) -> PlaybackCapabilities {
        PlaybackCapabilities {
            pause: !self.playback_controls_disabled.get(),
            seek: !self.playback_controls_disabled.get(),
            gapless: false,
            crossfade: false,
            speed: false,
            filters: false,
            audio_analysis: false,
        }
    }
    fn command<'a>(
        &'a self,
        command: PlaybackCommand,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            Playback::capabilities(self).check_command(&command)?;
            c.check_items(self.playback.borrow().len() + 1)?;
            self.playback.borrow_mut().push(command);
            Ok(())
        })
    }
    fn next_event<'a>(
        &'a self,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<PlaybackEvent, DomainError>> {
        Box::pin(async move {
            c.check(self.now())?;
            Ok(PlaybackEvent {
                context: c.correlation,
                occurrence: QueueOccurrenceId::new("1"),
                kind: PlaybackEventKind::Position(PlaybackPosition(Milliseconds(2))),
            })
        })
    }
}
#[test]
fn every_boundary_accepts_non_send_futures_and_deterministic_state() {
    let mut services = LocalServices::default();
    assert_eq!(services.next_u64(), 1);
    assert_eq!(services.now(), Milliseconds(1));
    let c = services.context();
    assert_eq!(ready(services.request(vec![1, 2], c)), Ok(vec![1, 2]));
    assert_eq!(
        ready(services.request(vec![0; 5], c)),
        Err(DomainError::Capacity)
    );
    let id = Scoped {
        server: ServerId::new("b"),
        id: SongId::new("s"),
    };
    let song = ready(services.song(id.clone(), c)).unwrap();
    assert_eq!(song.server, id.server);
    assert_eq!(song.id, id.id);
    assert_eq!(song.release_year, None);
    assert_eq!(song.duration, Milliseconds(1234));
    let q = SongListQuery {
        server: ServerId::new("b"),
        sort_by: SongListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 2,
        limit: Some(1),
        search_term: None,
        filter: SongFilter::default(),
    };
    let page = ready(services.songs(q, c)).unwrap();
    assert_eq!(page.start_index, 2);
    assert_eq!(page.total_record_count, None);
    assert_eq!(page.items.len(), 1);
    assert!(
        !ready(Backend::capabilities(&services, ServerId::new("b"), c))
            .unwrap()
            .has(ServerFeature::Jukebox)
    );
    ready(services.save("p", vec![7], c)).unwrap();
    assert_eq!(ready(services.load("p", c)), Ok(Some(vec![7])));
    ready(Persistence::remove(&services, "p", c)).unwrap();
    assert_eq!(ready(services.load("p", c)), Ok(None));
    ready(services.set("s", Secret::new(vec![9]), c)).unwrap();
    assert_eq!(ready(services.get("s", c)).unwrap().unwrap().expose(), &[9]);
    ready(SecretStore::remove(&services, "s", c)).unwrap();
    assert!(ready(services.get("s", c)).unwrap().is_none());
    assert!(Playback::capabilities(&services).seek);
    ready(services.command(PlaybackCommand::Seek(PlaybackPosition(Milliseconds(2))), c)).unwrap();
    assert_eq!(
        ready(services.command(PlaybackCommand::Pause, c)),
        Err(DomainError::Capacity)
    );
    let event = ready(services.next_event(c)).unwrap();
    assert_eq!(event.context, c.correlation);
    services.time.set(11);
    assert_eq!(
        ready(services.request(vec![], c)),
        Err(DomainError::Deadline)
    );
    services.cancelled.set(true);
    assert_eq!(
        ready(services.request(vec![], c)),
        Err(DomainError::Cancelled)
    );
}

fn catalog_vocabulary() -> String {
    let mut actual = String::new();
    macro_rules! catalog { ($($t:ty),+) => { $(
        actual.push_str(stringify!($t)); actual.push('\t');
        actual.push_str(&<$t>::ALL.iter().map(|v|v.as_str()).collect::<Vec<_>>().join(",")); actual.push('\n');
    )+ }; }
    catalog!(
        LibraryItem,
        ServerType,
        SortOrder,
        ExplicitStatus,
        ExternalSource,
        ExternalType,
        GenreListSort,
        ImageType,
        TagListSort,
        AlbumListSort,
        SongListSort,
        AlbumArtistListSort,
        ArtistListSort,
        PlaylistListSort,
        RadioListSort,
        UserListSort,
        Played,
        LyricSource,
        ServerFeature
    );
    actual
}

// Compare complete records without making Git checkout line endings part of the vocabulary.
// lines() removes LF/CRLF terminators only; spelling, ordering and whitespace stay significant.
fn catalog_records(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn catalog_vocabulary_matches_pinned_source_fixture() {
    assert_eq!(
        catalog_records(&catalog_vocabulary()),
        catalog_records(include_str!("../fixtures/p02/domain/catalog.tsv"))
    );
    for feature in ServerFeature::ALL {
        let mut caps = Capabilities::default();
        assert!(!caps.has(*feature));
        caps.0.insert(*feature, vec![]);
        assert!(!caps.has(*feature));
        caps.0.insert(*feature, vec![1, 3]);
        assert!(caps.has_version(*feature, 1));
        assert!(!caps.has_version(*feature, 2));
        assert!(!caps.has_version(*feature, 0));
    }
}

#[test]
fn checked_units_counters_and_limits_cover_edges() {
    assert_eq!(Seconds::new(-0.1), Err(UnitError::Negative));
    assert_eq!(Seconds::new(f64::INFINITY), Err(UnitError::NonFinite));
    assert_eq!(Seconds::new(0.0).unwrap().to_ticks(), Ok(Ticks(0)));
    assert_eq!(
        Seconds::new(1.0009).unwrap().to_milliseconds(),
        Ok(Milliseconds(1000))
    );
    assert_eq!(
        Milliseconds::from_seconds(u64::MAX as f64 / 1000.0),
        Err(UnitError::Overflow)
    );
    assert_eq!(
        Ticks::from_seconds(u64::MAX as f64),
        Err(UnitError::Overflow)
    );
    let max_ms = u64::MAX / 10_000;
    assert_eq!(Milliseconds(max_ms).to_ticks(), Ok(Ticks(max_ms * 10_000)));
    assert_eq!(
        Milliseconds(max_ms + 1).to_ticks(),
        Err(UnitError::Overflow)
    );
    assert_eq!(Revision(u64::MAX).next(), Err(DomainError::Exhausted));
    assert_eq!(Generation(u64::MAX).next(), Err(DomainError::Exhausted));
    assert_eq!(RequestId(u64::MAX).next(), Err(DomainError::Exhausted));
    let services = LocalServices::default();
    let c = services.context();
    assert_eq!(c.check_response_size(5), Err(DomainError::Capacity));
    assert_eq!(c.check_items(2), Err(DomainError::Capacity));
    assert_eq!(c.check(Milliseconds(10)), Ok(()));
    assert_eq!(c.check(Milliseconds(11)), Err(DomainError::Deadline));
}
#[test]
fn rejected_owner_events_are_terminal_and_do_not_mutate_state() {
    let mut owner = Owner::new(1);
    let Effect::Load { context } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(5),
        })
        .unwrap()
        .unwrap();
    let forged = RequestContext {
        request: RequestId(99),
        ..context
    };
    assert_eq!(
        owner.event(
            Event::Loaded {
                context: forged,
                items: vec![]
            },
            Milliseconds(0)
        ),
        Err(DomainError::Stale)
    );
    assert_eq!(
        owner.event(
            Event::LoadFailed {
                context,
                error: DomainError::Protocol
            },
            Milliseconds(0)
        ),
        Err(DomainError::Protocol)
    );
    assert_eq!(
        owner.event(
            Event::Loaded {
                context,
                items: vec![]
            },
            Milliseconds(0)
        ),
        Err(DomainError::Stale)
    );
    assert_eq!(owner.revision(), Revision(0));
    let Effect::Load { context } = owner
        .command(Command::BeginLoad {
            deadline: Milliseconds(5),
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        owner.event(
            Event::Loaded {
                context,
                items: vec![occurrence("1", "a"), occurrence("2", "a")]
            },
            Milliseconds(0)
        ),
        Err(DomainError::Capacity)
    );
    assert!(owner.items().is_empty());
    assert_eq!(owner.revision(), Revision(0));
    let mut owner = Owner::new(2);
    owner.command(Command::Add(occurrence("1", "a"))).unwrap();
    assert_eq!(
        owner.command(Command::Add(occurrence("1", "a"))),
        Err(DomainError::InvalidInput)
    );
    assert_eq!(owner.items().len(), 1);
    assert_eq!(owner.revision(), Revision(1));
}
#[test]
fn normalized_fields_preserve_unknown_empty_and_occurrence_context() {
    let mut first = sample_song();
    first.lyrics = Some(String::new());
    first.tags = Some(Default::default());
    first.is_resumable = Some(false);
    first.resume_position_ms = Some(PlaybackPosition(Milliseconds(0)));
    let second = sample_song();
    assert_ne!(first.lyrics, second.lyrics);
    assert_ne!(first.tags, second.tags);
    assert_ne!(first.is_resumable, second.is_resumable);
    let one = QueueSong {
        occurrence: QueueOccurrenceId::new("1"),
        song: first.clone(),
        context_playlist: Field::Absent,
    };
    let two = QueueSong {
        occurrence: QueueOccurrenceId::new("2"),
        song: first,
        context_playlist: Field::Null,
    };
    assert_eq!(one.song, two.song);
    assert_ne!(one.occurrence, two.occurrence);
    assert_ne!(one.context_playlist, two.context_playlist);
    let no_folders = SongFilter::default();
    let empty_folders = SongFilter {
        music_folder_ids: Some(vec![]),
        ..Default::default()
    };
    assert_ne!(no_folders, empty_folders);
}

#[test]
fn playback_capabilities_gate_dispatch_without_side_effects() {
    let services = LocalServices::default();
    let capabilities = Playback::capabilities(&services);
    for feature in [
        PlaybackCapability::Gapless,
        PlaybackCapability::Crossfade,
        PlaybackCapability::Speed,
        PlaybackCapability::Filters,
        PlaybackCapability::AudioAnalysis,
    ] {
        assert_eq!(capabilities.require(feature), Err(DomainError::Unsupported));
    }
    let gapless_engine = PlaybackCapabilities {
        gapless: true,
        ..capabilities
    };
    assert_eq!(gapless_engine.require(PlaybackCapability::Gapless), Ok(()));
    assert_eq!(
        gapless_engine.require(PlaybackCapability::Crossfade),
        Err(DomainError::Unsupported)
    );
    services.playback_controls_disabled.set(true);
    let context = services.context();
    for command in [
        PlaybackCommand::Pause,
        PlaybackCommand::Resume,
        PlaybackCommand::Seek(PlaybackPosition(Milliseconds(20))),
    ] {
        assert_eq!(
            ready(services.command(command, context)),
            Err(DomainError::Unsupported)
        );
        assert!(services.playback.borrow().is_empty());
    }
    services.playback_controls_disabled.set(false);
    ready(services.command(PlaybackCommand::Pause, context)).unwrap();
    assert_eq!(
        services.playback.borrow().as_slice(),
        &[PlaybackCommand::Pause]
    );
}

#[test]
fn catalog_fixture_line_endings_do_not_hide_vocabulary_changes() {
    let actual = catalog_vocabulary();
    let fixture = include_str!("../fixtures/p02/domain/catalog.tsv");
    let lf = fixture.lines().collect::<Vec<_>>().join("\n") + "\n";
    let crlf = lf.replace("\n", "\r\n");
    assert_ne!(
        actual, crlf,
        "the former raw comparison rejects a CRLF checkout"
    );
    for text in [&lf, &crlf] {
        assert_eq!(catalog_records(&actual), catalog_records(text));
        // The comparison must still reject renamed, missing, added and reordered values,
        // and must not trim meaningful whitespace inside a record.
        for altered in [
            text.replacen("album,albumArtist", "renamed,albumArtist", 1),
            text.replacen("album,albumArtist", "albumArtist", 1),
            text.replacen("album,albumArtist", "album,extra,albumArtist", 1),
            text.replacen("album,albumArtist", "albumArtist,album", 1),
            text.replacen("album,albumArtist", "album, albumArtist", 1),
        ] {
            assert_ne!(catalog_records(&actual), catalog_records(&altered));
        }
    }
}
