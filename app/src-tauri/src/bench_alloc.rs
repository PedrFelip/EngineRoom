//! Allocation traffic for explicitly instrumented test binaries only.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;

struct CountingAllocator;
#[derive(Default)]
struct Counters {
    calls: u64,
    bytes: u64,
    live: u64,
    peak: u64,
}
static COUNTERS: Mutex<Counters> = Mutex::new(Counters {
    calls: 0,
    bytes: 0,
    live: 0,
    peak: 0,
});
impl Counters {
    fn allocated(&mut self, size: u64) {
        self.calls += 1;
        self.bytes += size;
        self.live += size;
        self.peak = self.peak.max(self.live);
    }
    fn begin(&mut self) -> (u64, u64, u64) {
        self.peak = self.live;
        (self.calls, self.bytes, self.live)
    }
    fn finish(&self, start: (u64, u64, u64)) -> (u64, u64, u64) {
        (
            self.calls - start.0,
            self.bytes - start.1,
            self.peak.saturating_sub(start.2),
        )
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: All operations delegate unchanged pointers and layouts to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            COUNTERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .allocated(layout.size() as u64);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller supplies a live System allocation and its layout.
        unsafe { System.dealloc(ptr, layout) };
        COUNTERS.lock().unwrap_or_else(|e| e.into_inner()).live -= layout.size() as u64;
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies a live System allocation and valid new size.
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() {
            let mut counters = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
            counters.live -= layout.size() as u64;
            counters.allocated(size as u64);
        }
        result
    }
}

/// Starts an isolated measurement; run only one benchmark test at a time.
pub fn begin() -> (u64, u64, u64) {
    COUNTERS.lock().unwrap_or_else(|e| e.into_inner()).begin()
}

pub fn finish(start: (u64, u64, u64)) -> (u64, u64, u64) {
    COUNTERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .finish(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn sample_boundaries_and_concurrent_allocations_preserve_peak() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let worker = counters.clone();
        let thread = std::thread::spawn(move || {
            for _ in 0..10_000 {
                worker.lock().unwrap().allocated(128);
                std::thread::yield_now();
                worker.lock().unwrap().live -= 128;
            }
        });
        for _ in 0..10_000 {
            let start = counters.lock().unwrap().begin();
            std::thread::yield_now();
            let state = counters.lock().unwrap();
            let sample = state.finish(start);
            assert!(state.peak >= state.live);
            assert!(sample.2 >= state.live.saturating_sub(start.2));
        }
        thread.join().unwrap();
        let state = counters.lock().unwrap();
        assert_eq!(state.live, 0);
        assert_eq!(state.calls, 10_000);
        assert_eq!(state.bytes, 1_280_000);
    }
}
