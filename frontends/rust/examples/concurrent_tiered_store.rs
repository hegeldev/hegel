//! A two-tier key-value store: writes land in a memtable, and a flush
//! moves the memtable into the base table. Its contract is that writes and
//! flushes are serialized by the caller, while reads may run at any time
//! from any thread. The `read` rule is in both the `"write"` and the
//! `"flush"` group, so it overlaps each of them, while writes and flushes
//! never overlap each other.
//!
//! The flush has a window in which a key is in neither tier, so a read
//! during a flush can miss a key that was written long before. Only a
//! read that overlaps a flush can observe it.
//!
//! Run it with:
//!
//! ```text
//! cargo test --example concurrent_tiered_store
//! ```

use hegel::TestCase;
use hegel::generators as gs;
use hegel::stateful::{ConcurrentPool, concurrent_pool, machine};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

struct TieredStore {
    memtable: Mutex<HashMap<u64, i64>>,
    base: Mutex<HashMap<u64, i64>>,
}

fn lock(map: &Mutex<HashMap<u64, i64>>) -> std::sync::MutexGuard<'_, HashMap<u64, i64>> {
    map.lock().unwrap_or_else(|e| e.into_inner())
}

impl TieredStore {
    fn new() -> Self {
        TieredStore {
            memtable: Mutex::new(HashMap::new()),
            base: Mutex::new(HashMap::new()),
        }
    }

    fn put(&self, key: u64, value: i64) {
        lock(&self.memtable).insert(key, value);
    }

    fn get(&self, key: u64) -> Option<i64> {
        if let Some(value) = lock(&self.memtable).get(&key) {
            return Some(*value);
        }
        lock(&self.base).get(&key).copied()
    }

    fn flush(&self) {
        let flushed: Vec<(u64, i64)> = lock(&self.memtable).drain().collect();
        std::thread::yield_now(); // Makes it easier to find the race.
        let mut base = lock(&self.base);
        for (key, value) in flushed {
            base.insert(key, value);
        }
    }
}

struct TieredStoreTest {
    store: TieredStore,
    written: Mutex<HashSet<u64>>,
    keys: ConcurrentPool<u64>,
}

#[hegel::concurrent_state_machine]
impl TieredStoreTest {
    #[rule(groups = ["write"])]
    fn write(&self, tc: TestCase) {
        let key = tc.draw(gs::integers::<u64>().max_value(3));
        let value = tc.draw(gs::integers::<i64>());
        self.store.put(key, value);
        let first_write = self
            .written
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key);
        if first_write {
            self.keys.add(&tc, key);
        }
    }

    #[rule(groups = ["flush"])]
    fn flush(&self, _: TestCase) {
        self.store.flush();
    }

    #[rule(groups = ["write", "flush"])]
    fn read(&self, tc: TestCase) {
        let key = tc.draw(self.keys.values_reusable());
        let value = self.store.get(key);
        tc.note(&format!("read {key} -> {value:?}"));
        assert!(value.is_some(), "key {key} was written but is not readable");
    }
}

#[hegel::test]
fn test_concurrent_tiered_store(tc: TestCase) {
    let m = TieredStoreTest {
        store: TieredStore::new(),
        written: Mutex::new(HashSet::new()),
        keys: concurrent_pool(&tc),
    };
    machine(m).max_concurrency(4).run_concurrent(tc);
}

fn main() {}
