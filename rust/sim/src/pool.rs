//! Object pools reproducing the C++ object-list contracts.
//!
//! Two flavours mirror the original game's fixed-capacity containers, and their
//! iteration order is part of the deterministic state-hash contract that a later
//! task depends on, so it must match the C++ reference exactly.
//!
//! [`Pool`] mirrors `ExactObjectList<T, Limit>` (the `FixedObjectList` flavour
//! used by `wobjects`/`sobjects`/`nobjects`/bonuses): fixed capacity, a live/free
//! bitmap, allocation that always reuses the lowest free slot, and `iter()` that
//! yields live slots in slot (index) order.
//!
//! [`BloodPool`] mirrors `FastObjectList<T>` (the `BObjectList` flavour used by
//! `bobjects`): a contiguous run of live objects, append-on-spawn, and
//! swap-remove on free — including the free-during-iteration loop from
//! `Game::processFrame` (game.cpp), where freeing element `i` moves the last
//! live element into slot `i` and the iterator re-examines `i` without advancing.

// ---------------------------------------------------------------------------
// Pool<T> — ExactObjectList / FixedObjectList flavour
// ---------------------------------------------------------------------------

/// A fixed-capacity pool with live/free slot tracking.
///
/// Reproduces `ExactObjectList<T, Limit>`: [`spawn`](Pool::spawn) reuses the
/// lowest free slot index (matching the C++ bitmap scan), [`free`](Pool::free)
/// releases a slot, and [`iter`](Pool::iter) walks live slots in index order.
pub struct Pool<T> {
    slots: Vec<Option<T>>,
    len: usize,
}

impl<T> Pool<T> {
    /// Creates an empty pool with `capacity` slots.
    pub fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, || None);
        Pool { slots, len: 0 }
    }

    /// Total number of slots (fixed at construction).
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the pool holds no live objects.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts `value` into the lowest free slot, returning that slot index.
    ///
    /// Returns `None` when the pool is full (matching C++ `NewObject` returning
    /// `nullptr`). Reusing the lowest free index reproduces the deterministic
    /// allocation order of the C++ free-list bitmap scan.
    pub fn spawn(&mut self, value: T) -> Option<usize> {
        let idx = self.slots.iter().position(Option::is_none)?;
        self.slots[idx] = Some(value);
        self.len += 1;
        Some(idx)
    }

    /// Inserts `value`, mirroring C++ `ExactObjectList::NewObjectReuse`
    /// (`exactObjectList.hpp:57-67`; the `nobjects` list is
    /// `ExactObjectList<NObject, 600>`) and `FastObjectList::NewObjectReuse`
    /// (`fastObjectList.hpp:35-44`).
    ///
    /// While a free slot exists this is identical to [`spawn`](Pool::spawn) —
    /// the lowest free index, `len` incremented. When the pool is **full** it
    /// returns `&arr[limit-1]`: the **last slot** (index `capacity-1`) is
    /// overwritten *in place* and its index returned, with `len` left at the
    /// cap (no free, no swap). Unlike [`spawn`](Pool::spawn) (C++ `NewObject`,
    /// which returns `None` at cap) this never fails, so the death/blood spray
    /// storm keeps producing objects at the 600 cap exactly as C++ does.
    pub fn spawn_reuse(&mut self, value: T) -> usize {
        match self.slots.iter().position(Option::is_none) {
            Some(idx) => {
                self.slots[idx] = Some(value);
                self.len += 1;
                idx
            }
            None => {
                // Full: overwrite the last slot in place. `len` unchanged.
                let last = self.slots.len() - 1;
                self.slots[last] = Some(value);
                last
            }
        }
    }

    /// Frees the live object in `slot`. No-op if the slot is already free.
    pub fn free(&mut self, slot: usize) {
        if self.slots[slot].take().is_some() {
            self.len -= 1;
        }
    }

    /// Borrows the live object in `slot`, if any.
    pub fn get(&self, slot: usize) -> Option<&T> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// Mutably borrows the live object in `slot`, if any. The driver's
    /// per-tick object loop uses this to write a processed object back into its
    /// slot (the `Keep` outcome) after running it through `wobject_process`.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut T> {
        self.slots.get_mut(slot).and_then(Option::as_mut)
    }

    /// Iterates live objects in slot (index) order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().filter_map(Option::as_ref)
    }

    /// Iterates live objects mutably in slot (index) order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().filter_map(Option::as_mut)
    }
}

// ---------------------------------------------------------------------------
// BloodPool<T> — FastObjectList / BObjectList flavour
// ---------------------------------------------------------------------------

/// A fixed-capacity pool of contiguous live objects with swap-remove on free.
///
/// Reproduces `FastObjectList<T>`: [`spawn`](BloodPool::spawn) appends,
/// [`iter`](BloodPool::iter) walks the live run in order, and
/// [`retain_processing`](BloodPool::retain_processing) reproduces the C++
/// free-during-iteration loop where freeing the current element swaps the last
/// live element into its place and re-examines that slot.
pub struct BloodPool<T> {
    arr: Vec<T>,
    capacity: usize,
}

impl<T> BloodPool<T> {
    /// Creates an empty pool that can hold up to `capacity` live objects.
    pub fn new(capacity: usize) -> Self {
        BloodPool {
            arr: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Maximum number of live objects.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.arr.len()
    }

    /// Whether the pool holds no live objects.
    pub fn is_empty(&self) -> bool {
        self.arr.is_empty()
    }

    /// Appends `value` as a new live object, returning its current slot index.
    ///
    /// Returns `None` when the pool is full (matching C++ `NewObject`).
    pub fn spawn(&mut self, value: T) -> Option<usize> {
        if self.arr.len() == self.capacity {
            return None;
        }
        let idx = self.arr.len();
        self.arr.push(value);
        Some(idx)
    }

    /// Frees the object at `index` via swap-remove: the last live object takes
    /// its place. Matches C++ `Free(ptr)` (`*ptr = arr[--count]`).
    pub fn free(&mut self, index: usize) {
        self.arr.swap_remove(index);
    }

    /// Iterates live objects in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.arr.iter()
    }

    /// Iterates live objects mutably in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.arr.iter_mut()
    }

    /// Walks every live object in slot order, freeing those for which `keep`
    /// returns `false` via swap-remove.
    ///
    /// This reproduces the `Game::processFrame` blood loop exactly: a freed slot
    /// receives the last live element and is re-examined without advancing, so
    /// surviving order is *not* preserved when a non-final element is freed.
    pub fn retain_processing<F: FnMut(&mut T) -> bool>(&mut self, mut keep: F) {
        let mut i = 0;
        while i < self.arr.len() {
            if keep(&mut self.arr[i]) {
                i += 1;
            } else {
                self.arr.swap_remove(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pool<T> (FixedObjectList) -----------------------------------------

    #[test]
    fn new_pool_is_empty_and_iter_yields_nothing() {
        let pool: Pool<i32> = Pool::new(8);
        assert_eq!(pool.capacity(), 8);
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
        assert_eq!(pool.iter().count(), 0);
    }

    #[test]
    fn spawn_k_yields_slot_order_and_len() {
        let mut pool: Pool<i32> = Pool::new(8);
        assert_eq!(pool.spawn(10), Some(0));
        assert_eq!(pool.spawn(20), Some(1));
        assert_eq!(pool.spawn(30), Some(2));
        assert_eq!(pool.spawn(40), Some(3));
        assert_eq!(pool.spawn(50), Some(4));

        assert_eq!(pool.len(), 5);
        let live: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(live, vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn free_middle_slot_preserves_survivor_order_and_reuses_lowest_free_index() {
        let mut pool: Pool<i32> = Pool::new(4);
        pool.spawn(10);
        pool.spawn(20);
        pool.spawn(30);

        // Free the middle slot.
        pool.free(1);
        assert_eq!(pool.len(), 2);
        let survivors: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(survivors, vec![10, 30]); // order preserved, slot 1 skipped

        // Next spawn reuses the lowest free index (slot 1), matching C++.
        assert_eq!(pool.spawn(40), Some(1));
        assert_eq!(pool.len(), 3);
        let after: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(after, vec![10, 40, 30]);
    }

    #[test]
    fn spawn_returns_none_when_full() {
        let mut pool: Pool<i32> = Pool::new(2);
        assert_eq!(pool.spawn(1), Some(0));
        assert_eq!(pool.spawn(2), Some(1));
        assert_eq!(pool.spawn(3), None);
        assert_eq!(pool.len(), 2);
    }

    // O3 — `spawn_reuse` mirrors C++ `ExactObjectList::NewObjectReuse`
    // (`exactObjectList.hpp:57-67`) / `FastObjectList::NewObjectReuse`
    // (`fastObjectList.hpp:35-44`): behaves like `spawn` while a free slot
    // exists, but at capacity overwrites the LAST slot (index `capacity-1`) in
    // place and returns it — `len` unchanged, no free, no swap.

    #[test]
    fn spawn_reuse_under_cap_matches_spawn() {
        // Below capacity `spawn_reuse` takes the `GetFreeObject` branch: lowest
        // free index, `len` increments — byte-identical to `spawn`.
        let mut pool: Pool<i32> = Pool::new(4);
        assert_eq!(pool.spawn_reuse(10), 0);
        assert_eq!(pool.spawn_reuse(20), 1);
        assert_eq!(pool.len(), 2);
        let live: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(live, vec![10, 20]);
    }

    #[test]
    fn spawn_reuse_full_pool_overwrites_last_slot_in_place() {
        // Fill a full 600-slot pool (the C++ `nobjects` limit).
        let mut pool: Pool<i32> = Pool::new(600);
        for v in 0..600 {
            assert_eq!(pool.spawn_reuse(v), v as usize);
        }
        assert_eq!(pool.len(), 600);

        // At cap, `spawn_reuse` returns `limit-1` (matching C++ `&arr[limit-1]`),
        // overwrites that slot's VALUE in place, and leaves `len` at the cap.
        assert_eq!(pool.spawn_reuse(-1), 599);
        assert_eq!(pool.len(), 600, "count stays at limit (no free/swap)");
        assert_eq!(pool.get(599), Some(&-1), "last slot value replaced");
        // No swap: the previous-to-last slot is untouched (a swap-remove would
        // have moved a survivor here) and slot 0 is untouched.
        assert_eq!(pool.get(598), Some(&598));
        assert_eq!(pool.get(0), Some(&0));

        // A second overwrite hits the same slot again — still no growth.
        assert_eq!(pool.spawn_reuse(-2), 599);
        assert_eq!(pool.len(), 600);
        assert_eq!(pool.get(599), Some(&-2));
    }

    #[test]
    fn spawn_vs_spawn_reuse_at_cap() {
        // Contrast: `spawn` (C++ `NewObject`, `game.cpp:244-246`) bails with
        // `None` at cap and leaves the pool unchanged, whereas `spawn_reuse`
        // (C++ `NewObjectReuse`) always yields the last slot.
        let mut pool: Pool<i32> = Pool::new(2);
        pool.spawn_reuse(1);
        pool.spawn_reuse(2);
        assert_eq!(pool.len(), 2);

        assert_eq!(pool.spawn(3), None, "NewObject returns None at cap");
        assert_eq!(pool.len(), 2);
        let before: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(before, vec![1, 2]);

        assert_eq!(pool.spawn_reuse(3), 1, "NewObjectReuse overwrites limit-1");
        assert_eq!(pool.len(), 2);
        let after: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(after, vec![1, 3]);
    }

    #[test]
    fn double_free_is_noop() {
        let mut pool: Pool<i32> = Pool::new(4);
        pool.spawn(7);
        pool.free(0);
        pool.free(0); // already free
        assert_eq!(pool.len(), 0);
        assert!(pool.get(0).is_none());
    }

    // --- BloodPool<T> (BObjectList) ----------------------------------------

    #[test]
    fn blood_new_is_empty() {
        let pool: BloodPool<i32> = BloodPool::new(16);
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
        assert_eq!(pool.iter().count(), 0);
    }

    #[test]
    fn blood_spawn_appends_in_slot_order() {
        let mut pool: BloodPool<i32> = BloodPool::new(16);
        assert_eq!(pool.spawn(1), Some(0));
        assert_eq!(pool.spawn(2), Some(1));
        assert_eq!(pool.spawn(3), Some(2));
        assert_eq!(pool.len(), 3);
        let live: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(live, vec![1, 2, 3]);
    }

    #[test]
    fn blood_spawn_full_returns_none() {
        let mut pool: BloodPool<i32> = BloodPool::new(1);
        assert_eq!(pool.spawn(1), Some(0));
        assert_eq!(pool.spawn(2), None);
    }

    #[test]
    fn blood_free_during_iteration_uses_swap_remove_order() {
        // Mirrors the Game::processFrame blood loop: keep odd values, free even.
        // Swap-remove means the last live element jumps into a freed slot, so
        // surviving order is NOT the original order — this pins the C++ contract.
        let mut pool: BloodPool<i32> = BloodPool::new(8);
        for v in [1, 2, 3, 4, 5] {
            pool.spawn(v);
        }

        pool.retain_processing(|v| *v % 2 == 1);

        // Trace of swap-remove:
        //   [1,2,3,4,5] free 2 -> [1,5,3,4]   (5 swapped into slot 1)
        //   [1,5,3,4]   free 4 -> [1,5,3]
        let survivors: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(survivors, vec![1, 5, 3]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn blood_free_last_element_is_plain_pop() {
        let mut pool: BloodPool<i32> = BloodPool::new(8);
        for v in [1, 2, 3] {
            pool.spawn(v);
        }
        pool.free(2); // last element: no swap, just shrink
        let live: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(live, vec![1, 2]);
    }
}
