//! Deferred (bottom-half) work queue.
//!
//! Interrupt handlers enqueue `fn()` thunks; a safe context drains them.
//! Today the kernel idle paths drain after every wake-up, bounding latency
//! by the tick rate without nesting work inside another ISR. When the async
//! executor lands, draining moves into task context unchanged.

use spin::Mutex;

/// Ring capacity. Overflowing rejects the enqueue and returns `false` so the
/// ISR can drop or count the event instead of blocking.
const CAPACITY: usize = 64;

/// Cap on thunks run per drain to keep a self-re-enqueueing thunk from
/// starving the idle loop forever.
const DRAIN_BATCH_CAP: usize = 256;

struct Queue {
    slots: [Option<fn()>; CAPACITY],
    next_read: usize,
    next_write: usize,
    len: usize,
}

static QUEUE: Mutex<Queue> = Mutex::new(Queue {
    slots: [const { None }; CAPACITY],
    next_read: 0,
    next_write: 0,
    len: 0,
});

/// Enqueues `f` to run at the next drain point.
///
/// Returns `false` when the ring is full — the thunk is dropped.
pub fn enqueue(f: fn()) -> bool {
    let mut q = QUEUE.lock();
    if q.len >= CAPACITY {
        return false;
    }
    let w = q.next_write;
    q.slots[w] = Some(f);
    q.next_write = (w + 1) % CAPACITY;
    q.len += 1;
    true
}

/// Runs every currently queued thunk, including ones enqueued while
/// draining (bounded per call). Safe to call from any context that may take
/// locks; never call from an ISR holding a lock a thunk needs.
pub fn drain() {
    for _ in 0..DRAIN_BATCH_CAP {
        // Pop under the lock, run outside it so thunks may re-enqueue.
        let thunk = {
            let mut q = QUEUE.lock();
            if q.len == 0 {
                return;
            }
            let f = {
                let r = q.next_read;
                q.slots[r].take()
            };
            q.next_read = (q.next_read + 1) % CAPACITY;
            q.len -= 1;
            f
        };
        if let Some(f) = thunk {
            f();
        }
    }
    log::warn!("bottom_half: drain batch cap hit; deferring remainder");
}
