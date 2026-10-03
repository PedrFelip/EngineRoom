use super::*;
use crate::review::pool::Pool;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
struct Activity {
    active: AtomicUsize,
    peak: AtomicUsize,
    closed: AtomicUsize,
    fail: AtomicBool,
    stall: AtomicBool,
    searches: Mutex<Vec<(String, u32)>>,
    delays: Mutex<HashMap<String, u64>>,
}
struct GroupFactory {
    state: Arc<Mutex<FakeState>>,
    activity: Arc<Activity>,
    permit: Arc<Semaphore>,
}
struct GroupPort {
    inner: FakePort,
    activity: Arc<Activity>,
    lease: Option<Arc<OwnedSemaphorePermit>>,
    searching: bool,
    delay: bool,
}
impl EngineFactory for GroupFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move { Ok(self.acquire_pool(1, cancel).await?.pop().unwrap()) })
    }
    fn acquire_pool<'a>(
        &'a self,
        count: usize,
        cancel: &'a Cancellation,
    ) -> Task<'a, Result<Vec<Box<dyn EnginePort>>>> {
        Box::pin(async move {
            let lease = tokio::select! {
                _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
                lease = self.permit.clone().acquire_owned() => Arc::new(lease.unwrap()),
            };
            Ok((0..count)
                .map(|_| {
                    Box::new(GroupPort {
                        inner: FakePort {
                            state: self.state.clone(),
                            queue: VecDeque::new(),
                            fen: String::new(),
                            permit: None,
                        },
                        activity: self.activity.clone(),
                        lease: Some(lease.clone()),
                        searching: false,
                        delay: false,
                    }) as Box<dyn EnginePort>
                })
                .collect())
        })
    }
}
impl EnginePort for GroupPort {
    fn send(&mut self, command: &str) -> Result<()> {
        self.inner.send(command)?;
        if command.starts_with("go ") {
            if let Some(value) = command.strip_prefix("go movetime ") {
                self.activity.searches.lock().unwrap().push((
                    self.inner.fen.clone(),
                    value.split_whitespace().next().unwrap().parse().unwrap(),
                ));
            }
            self.searching = true;
            self.delay = true;
            let active = self.activity.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.activity.peak.fetch_max(active, Ordering::SeqCst);
        }
        Ok(())
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            if self.delay {
                self.delay = false;
                let delay = self
                    .activity
                    .delays
                    .lock()
                    .unwrap()
                    .get(&self.inner.fen)
                    .copied()
                    .unwrap_or(10);
                tokio::time::sleep(Duration::from_millis(delay)).await;
                if self.activity.fail.load(Ordering::SeqCst) {
                    return Err(ReviewError::new(
                        ReviewErrorCode::EngineExited,
                        "test.pool",
                        "worker failed",
                    ));
                }
                if self.activity.stall.load(Ordering::SeqCst) {
                    std::future::pending::<()>().await;
                }
            }
            let line = self.inner.next().await?;
            if line.starts_with("bestmove") && self.searching {
                self.searching = false;
                self.activity.active.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(line)
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            // Model acknowledged teardown, with the lease held throughout.
            tokio::time::sleep(Duration::from_millis(10)).await;
            if self.searching {
                self.searching = false;
                self.activity.active.fetch_sub(1, Ordering::SeqCst);
            }
            self.inner.shutdown().await;
            self.activity.closed.fetch_add(1, Ordering::SeqCst);
            self.lease.take();
        })
    }
}
fn group() -> Arc<GroupFactory> {
    Arc::new(GroupFactory {
        state: Arc::new(Mutex::new(FakeState::default())),
        activity: Arc::new(Activity::default()),
        permit: Arc::new(Semaphore::new(1)),
    })
}

#[tokio::test]
async fn submitted_searches_run_concurrently_and_preserve_hash_on_multipv_change() {
    let factory = group();
    let (_close, cancel) = cancel();
    let game = core::extract("1. e4 e5 2. Nf3 Nc6").unwrap();
    let mut pool = Pool::acquire(factory.as_ref(), Some((6, 512)), game.fens.len(), &cancel)
        .await
        .unwrap();
    assert_eq!(pool.len(), 3);
    for multipv in [1, 3] {
        let mut results = vec![];
        for (index, fen) in game.fens.iter().enumerate() {
            results.push(
                pool.submit(index % pool.len(), fen.clone(), 100, multipv)
                    .await
                    .unwrap(),
            );
        }
        for (fen, receiver) in game.fens.iter().zip(results) {
            let result = receiver.await.unwrap().unwrap();
            assert_eq!(result.raw.fen, *fen);
            assert_eq!(result.value, 100);
        }
    }
    assert_eq!(factory.activity.peak.load(Ordering::SeqCst), 3);
    let sent = factory.state.lock().unwrap().sent.clone();
    assert_eq!(
        sent.iter()
            .filter(|s| s.starts_with("setoption name Threads "))
            .count(),
        3
    );
    let hash: Vec<u32> = sent
        .iter()
        .filter_map(|s| {
            s.strip_prefix("setoption name Hash value ")
                .map(|s| s.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(hash.iter().sum::<u32>(), 512);
    assert_eq!(hash.len(), 3);
    assert_eq!(factory.permit.available_permits(), 0);
    pool.close().await;
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 3);
    assert_eq!(factory.permit.available_permits(), 1);
    assert_eq!(factory.activity.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn automatic_worker_failure_closes_every_process_before_next_owner() {
    let factory = group();
    factory.activity.fail.store(true, Ordering::SeqCst);
    let mut pipeline = Pipeline::new(factory.clone(), Arc::new(MemoryRepo::default()));
    let (_closed, cancel) = cancel();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        pipeline.review(
            &config("1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6", AnalysisKind::Fast),
            Some((6, 512)),
            &cancel,
            &mut |_| {},
        ),
    )
    .await
    .unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code, ReviewErrorCode::EngineExited);
    assert_eq!(error.message, "worker failed");
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 3);
    assert_eq!(factory.activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(factory.permit.available_permits(), 1);
    let mut next_owner = factory.acquire(&cancel).await.unwrap();
    next_owner.shutdown().await;
    assert_eq!(factory.permit.available_permits(), 1);
}

#[tokio::test]
async fn real_sidecar_pool_holds_one_lease_until_all_processes_terminate() {
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_shell::init())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let permit = Arc::new(Semaphore::new(1));
    let factory = SidecarFactory {
        app: app.handle().clone(),
        permit: permit.clone(),
    };
    let (_close, cancel) = cancel();
    let fens = core::extract("1. e4 e5").unwrap().fens;
    let mut pool = Pool::acquire(&factory, Some((6, 96)), fens.len(), &cancel)
        .await
        .unwrap();
    assert_eq!(pool.len(), 3);
    assert_eq!(permit.available_permits(), 0);
    let mut results = vec![];
    for (worker, fen) in fens.iter().enumerate() {
        results.push(pool.submit(worker, fen.clone(), 100, 1).await.unwrap());
    }
    for (fen, receiver) in fens.iter().zip(results) {
        let result = receiver.await.unwrap().unwrap();
        assert_eq!(result.raw.fen, *fen);
        assert!(result.raw.depth > 0);
        assert_eq!(result.value, 100);
        assert_eq!(result.raw.lines.len(), 1);
    }
    // Probe must wait even when every search has completed: the processes
    // and their transposition tables still belong to the review.
    assert!(
        tokio::time::timeout(Duration::from_millis(20), factory.acquire(&cancel))
            .await
            .is_err()
    );
    pool.close().await;
    assert_eq!(permit.available_permits(), 1);
    let mut probe = factory.acquire(&cancel).await.unwrap();
    ask(probe.as_mut(), "uci", "uciok", 10000, &cancel)
        .await
        .unwrap();
    probe.shutdown().await;
    assert_eq!(permit.available_permits(), 1);
}

#[tokio::test]
async fn parallel_pipeline_keeps_profile_budgets_and_failure_flush_keeps_original_error() {
    for fail_write in [false, true] {
        let factory = group();
        let repo = Arc::new(MemoryRepo {
            fail_put: fail_write,
            ..Default::default()
        });
        let mut pipeline = Pipeline::new(factory.clone(), repo.clone());
        let (_close, cancel) = cancel();
        let pgn = "1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7";
        let mut events = vec![];
        let result = pipeline
            .review(
                &config(pgn, AnalysisKind::Fast),
                Some((6, 512)),
                &cancel,
                &mut |event| events.push(event),
            )
            .await;
        if fail_write {
            assert_eq!(result.unwrap_err().message, "cache write failed");
        } else {
            assert_eq!(result.unwrap().positions.len(), 11);
            let sent = factory.state.lock().unwrap().sent.clone();
            let budgets: Vec<u32> = sent
                .iter()
                .filter_map(|s| {
                    s.strip_prefix("go movetime ")
                        .map(|s| s.split_whitespace().next().unwrap().parse().unwrap())
                })
                .collect();
            assert!(budgets.iter().all(|value| [180, 500, 2000].contains(value)));
            let writes = repo.writes.lock().unwrap();
            assert!(!writes.is_empty());
            assert!(writes
                .iter()
                .all(|(value, _)| [180, 500, 2000].contains(value)));
            assert!(sent
                .iter()
                .filter(|s| s.starts_with("go ")
                    && !s.starts_with("go movetime 180 ")
                    && s.as_str() != "go movetime 180")
                .all(|s| !s.contains(" searchmoves ")));
            let indexes: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Progress { progress } if progress.stage == "triage" => {
                        progress.update.as_ref().map(|_| progress.current_ply)
                    }
                    _ => None,
                })
                .collect();
            let mut unique = indexes.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique, (0..11).collect::<Vec<_>>());
        }
        assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 3);
        assert_eq!(factory.permit.available_permits(), 1);
    }
}

#[tokio::test]
async fn mixed_queue_starts_critical_refinements_before_triage_finishes() {
    let factory = group();
    let pgn = "1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7";
    let game = core::extract(pgn).unwrap();
    for (i, fen) in game.fens.iter().enumerate() {
        let cp = if i == 1 { 500 } else { 0 };
        let mut raw = core::terminal_raw(fen, cp);
        raw.lines = vec![RawLine {
            unstable: false,
            multipv: 1,
            cp,
            pv: vec![],
            san: None,
            depth: Some(12),
        }];
        factory
            .state
            .lock()
            .unwrap()
            .scores
            .insert(fen.clone(), raw);
        factory.activity.delays.lock().unwrap().insert(
            fen.clone(),
            if i == 0 {
                30
            } else if (4..8).contains(&i) {
                25
            } else {
                1
            },
        );
    }
    let repo = Arc::new(MemoryRepo::default());
    let mut pipeline = Pipeline::new(factory.clone(), repo.clone());
    let (_closed, cancel) = cancel();
    let mut updates = vec![];
    pipeline
        .review(
            &config(pgn, AnalysisKind::Fast),
            Some((6, 512)),
            &cancel,
            &mut |event| {
                if let Event::Progress { progress } = event {
                    if let Some(update) = progress.update {
                        updates.push((update.index, progress.completed, progress.stage));
                    }
                }
            },
        )
        .await
        .unwrap();
    let searches = factory.activity.searches.lock().unwrap();
    let first_refinement = searches
        .iter()
        .position(|(_, value)| *value == 2000)
        .unwrap();
    let last_triage = searches
        .iter()
        .rposition(|(_, value)| *value == 180)
        .unwrap();
    assert!(first_refinement < last_triage, "{searches:?}");
    let mut first_block_starts: Vec<_> = searches[..3]
        .iter()
        .map(|(fen, _)| game.fens.iter().position(|f| f == fen).unwrap())
        .collect();
    first_block_starts.sort_unstable();
    assert_eq!(first_block_starts, vec![0, 4, 8]);
    assert_eq!(updates[0].0, 8); // No waiting for the slower position zero.
    assert!(updates
        .iter()
        .any(|(i, done, stage)| *i <= 2 && *done < 11 && stage == "triage"));
    let expected = adaptive::review_targets(
        &game,
        &game
            .fens
            .iter()
            .map(|f| factory.state.lock().unwrap().scores[f].clone())
            .collect::<Vec<_>>(),
        adaptive::profile(AnalysisKind::Fast).unwrap(),
    );
    let mut refined: Vec<_> = searches
        .iter()
        .filter(|(_, value)| *value != 180)
        .map(|(fen, _)| game.fens.iter().position(|f| f == fen).unwrap())
        .collect();
    refined.sort_unstable();
    let mut selected: Vec<_> = expected.iter().map(|t| t.position_index).collect();
    selected.sort_unstable();
    assert_eq!(refined, selected); // Shared targets are refined once; final quota is unchanged.
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 3);
    assert_eq!(
        repo.writes
            .lock()
            .unwrap()
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>(),
        searches.len()
    );
}

#[tokio::test]
async fn manual_mode_uses_one_engine_even_with_large_resource_budget() {
    let factory = group();
    let mut pipeline = Pipeline::new(factory.clone(), Arc::new(MemoryRepo::default()));
    let (_closed, cancel) = cancel();
    pipeline
        .review(
            &config("1. e4 e5 2. Nf3 Nc6", AnalysisKind::Manual),
            Some((12, 512)),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
    assert_eq!(factory.activity.peak.load(Ordering::SeqCst), 1);
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 1);
    let sent = &factory.state.lock().unwrap().sent;
    assert!(sent.contains(&"setoption name Threads value 12".into()));
    assert!(sent.contains(&"setoption name Hash value 512".into()));
}

#[tokio::test]
async fn mixed_review_cancellation_awaits_every_worker() {
    let factory = group();
    factory.activity.stall.store(true, Ordering::SeqCst);
    let mut pipeline = Pipeline::new(factory.clone(), Arc::new(MemoryRepo::default()));
    let (closed, cancel) = cancel();
    let activity = factory.activity.clone();
    let cancel_task = tokio::spawn(async move {
        while activity.active.load(Ordering::SeqCst) < 3 {
            tokio::task::yield_now().await;
        }
        closed.send_replace(true);
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        pipeline.review(
            &config(
                "1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7",
                AnalysisKind::Deep,
            ),
            Some((6, 512)),
            &cancel,
            &mut |_| {},
        ),
    )
    .await
    .unwrap();
    cancel_task.await.unwrap();
    assert_eq!(result.unwrap_err().code, ReviewErrorCode::Cancelled);
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 3);
    assert_eq!(factory.activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(factory.permit.available_permits(), 1);
}

#[tokio::test]
async fn fully_cached_automatic_review_never_acquires_engines() {
    let factory = group();
    let repo = Arc::new(MemoryRepo::default());
    let pgn = "1. a3 a6 2. h3 h6";
    let game = core::extract(pgn).unwrap();
    for (i, fen) in game.fens.iter().enumerate() {
        repo.hits.lock().unwrap().insert(
            fen.clone(),
            core::terminal_raw(fen, if i == 1 { 500 } else { 0 }),
        );
    }
    let mut pipeline = Pipeline::new(factory.clone(), repo.clone());
    let (_closed, cancel) = cancel();
    pipeline
        .review(
            &config(pgn, AnalysisKind::Deep),
            Some((6, 512)),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
    assert_eq!(factory.activity.closed.load(Ordering::SeqCst), 0);
    assert_eq!(factory.state.lock().unwrap().searches, 0);
    assert!(repo.writes.lock().unwrap().is_empty());
    assert_eq!(factory.permit.available_permits(), 1);
}
