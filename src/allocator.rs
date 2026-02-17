use crate::heap::GlobalHeap;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = usize::MAX - HEAP_START;

#[global_allocator]
static HEAP: GlobalHeap = GlobalHeap::new();

pub fn init() {
    HEAP.init(HEAP_START, HEAP_SIZE);
    log::info!("Heap allocator initialized");
}
