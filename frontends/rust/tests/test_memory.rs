//! Memory a long-running test case keeps, measured as the live heap bytes
//! of this test binary. The counting allocator sees the engine's
//! allocations only when the engine is linked in (`static-engine`), and the
//! fixture runs in a subprocess of its own (`exec::self_test`) so that no
//! other test allocates while it measures.

#![cfg(all(feature = "static-engine", not(windows)))]

mod common;

use common::exec::self_test;
use hegel::TestCase;
use hegel::generators as gs;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use tempfile::TempDir;

struct Counting;

static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            LIVE_BYTES.fetch_add(
                new_size as isize - layout.size() as isize,
                Ordering::Relaxed,
            );
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const STEPS: usize = 50_000;
const SAMPLES: usize = 10;

const UNSAMPLED: isize = -1;

static CHECKS: AtomicUsize = AtomicUsize::new(0);
static SAMPLED: [AtomicIsize; SAMPLES] = [const { AtomicIsize::new(UNSAMPLED) }; SAMPLES];

struct Idle;

#[hegel::concurrent_state_machine]
impl Idle {
    #[rule]
    fn idle(&self, tc: TestCase) {
        tc.draw(gs::weighted_booleans(0.02));
    }

    #[invariant(always_run)]
    fn sample(&self, _tc: TestCase) {
        let checks = CHECKS.fetch_add(1, Ordering::Relaxed) + 1;
        let index = checks / (STEPS / SAMPLES);
        if checks % (STEPS / SAMPLES) == 0 && index <= SAMPLES {
            SAMPLED[index - 1].store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    }
}

#[hegel::test(test_cases = 1, phases = [hegel::Phase::Generate])]
#[ignore = "fixture: run via exec::self_test"]
fn long_running_machine_fixture(tc: TestCase) {
    hegel::stateful::machine(Idle)
        .max_concurrency(1)
        .steps(STEPS as i64)
        .run_concurrent(tc);
    let samples: Vec<String> = SAMPLED
        .iter()
        .map(|sample| sample.load(Ordering::Relaxed).to_string())
        .collect();
    eprintln!("live bytes: {}", samples.join(" "));
}

/// A single test case inside Antithesis runs unrecorded (a workload), so
/// however many rounds its state machine runs, the heap it keeps must stay
/// flat.
#[test]
fn a_long_running_unrecorded_machine_keeps_a_flat_heap() {
    let output_dir = TempDir::new().unwrap();
    let output = self_test("long_running_machine_fixture")
        .env("ANTITHESIS_OUTPUT_DIR", output_dir.path().to_str().unwrap())
        .run();
    let samples: Vec<isize> = output
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("live bytes: "))
        .unwrap_or_else(|| panic!("no samples in the fixture's output:\n{}", output.stderr))
        .split(' ')
        .map(|sample| sample.parse().unwrap())
        .collect();
    assert_eq!(samples.len(), SAMPLES);
    assert!(
        !samples.contains(&UNSAMPLED),
        "the machine stopped before every sample was taken: {samples:?}"
    );
    let growth = samples[SAMPLES - 1] - samples[1];
    assert!(
        growth < 64 * 1024,
        "the heap grew by {growth} bytes over {} rounds: {samples:?}",
        STEPS - STEPS / SAMPLES * 2
    );
}
