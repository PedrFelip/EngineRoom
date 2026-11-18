//! Targeted A/B harness for the flat-indexed review pipeline structures.
//!
//! Uses only public APIs present before and after the change so the same
//! source compiles against both revisions. Run in release mode:
//!
//! ```text
//! cargo build --release --example bench_flat -p engineroom-application --locked
//! ./target/release/examples/bench_flat [multiplier]
//! ```
//!
//! Allocations are counted process-wide with a wrapping `System` allocator and
//! reported as deltas around each workload; they include the harness loop but
//! exclude setup. Times are wall clock on a single current-thread runtime.

use engineroom_application::{
    db::mode::Mode,
    review::{
        adaptive::{self, Critical, Reason},
        engine::{self, Cancellation, EnginePort, Task},
        types::{AnalysisKind, ReviewError},
    },
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
use tokio::sync::watch;

struct CountingAllocator;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if !next.is_null() {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        next
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct Counters {
    allocs: usize,
    bytes: usize,
}
fn counters() -> Counters {
    Counters {
        allocs: ALLOCS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    }
}

struct FakePort {
    lines: Arc<Vec<String>>,
    cursor: usize,
}
impl FakePort {
    fn new(lines: &Arc<Vec<String>>) -> Self {
        Self {
            lines: lines.clone(),
            cursor: 0,
        }
    }
}
impl EnginePort for FakePort {
    fn send<'a>(&'a mut self, _: &'a str) -> Task<'a, Result<(), ReviewError>> {
        Box::pin(async { Ok(()) })
    }
    fn next(&mut self) -> Task<'_, Result<String, ReviewError>> {
        Box::pin(async move {
            let line = self
                .lines
                .get(self.cursor)
                .cloned()
                .unwrap_or_else(|| "bestmove e2e4".into());
            self.cursor += 1;
            Ok(line)
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}

fn uci_lines(ranks: u32, depths: u32) -> Arc<Vec<String>> {
    let mut lines = Vec::with_capacity((ranks * depths) as usize + 1);
    for depth in 1..=depths {
        for rank in 1..=ranks {
            let cp = (depth as i32 * 7) - (rank as i32 * 11);
            lines.push(format!(
                "info depth {depth} multipv {rank} score cp {cp} pv e2e4 e7e5 g1f3 b8c6"
            ));
        }
    }
    lines.push("bestmove e2e4".into());
    Arc::new(lines)
}

fn cancel() -> Cancellation {
    let (_tx, rx) = watch::channel(false);
    Cancellation {
        closed: rx,
        revision: None,
    }
}

fn criticals(total: usize) -> Vec<Critical> {
    let reasons = [
        Reason::Loss,
        Reason::Swing,
        Reason::Tactical,
        Reason::Complex,
        Reason::ClassificationBoundary,
        Reason::UnstableSearch,
        Reason::Mate,
    ];
    (0..total)
        .map(|i| Critical {
            ply: i % 200 + 1,
            score: (i * 37 % 90 + 20) as u32,
            hard: i % 9 == 0,
            reasons: vec![reasons[i % reasons.len()]],
        })
        .collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scale: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let cancel = cancel();

    // Workload 1: UCI MultiPV accumulation through the production evaluate().
    let lines = uci_lines(4, 24);
    let uci_iters = 1500 * scale;
    for _ in 0..20 {
        let mut warm = FakePort::new(&lines);
        engine::evaluate(&mut warm, fen, Mode::Depth, 12, 5000, &cancel).await?;
    }
    let before = counters();
    let start = Instant::now();
    for _ in 0..uci_iters {
        let mut port = FakePort::new(&lines);
        engine::evaluate(&mut port, fen, Mode::Depth, 12, 5000, &cancel).await?;
    }
    let uci_time = start.elapsed();
    let after = counters();

    // Workload 2: dense adaptive target selection.
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let critics = criticals(220);
    let target_iters = 8000 * scale;
    for _ in 0..100 {
        let _ = adaptive::targets(&critics, 240, profile);
    }
    let before_targets = counters();
    let start = Instant::now();
    let mut sink = 0usize;
    for _ in 0..target_iters {
        sink += adaptive::targets(&critics, 240, profile).len();
    }
    let target_time = start.elapsed();
    let after_targets = counters();
    std::hint::black_box(sink);

    println!("scale={scale}");
    println!(
        "uci: iters={} total_ms={:.3} ns_per_op={:.1} allocs={} bytes={}",
        uci_iters,
        uci_time.as_secs_f64() * 1000.0,
        uci_time.as_nanos() as f64 / uci_iters as f64,
        after.allocs - before.allocs,
        after.bytes - before.bytes,
    );
    println!(
        "targets: iters={} total_ms={:.3} ns_per_op={:.1} allocs={} bytes={}",
        target_iters,
        target_time.as_secs_f64() * 1000.0,
        target_time.as_nanos() as f64 / target_iters as f64,
        after_targets.allocs - before_targets.allocs,
        after_targets.bytes - before_targets.bytes,
    );
    Ok(())
}
