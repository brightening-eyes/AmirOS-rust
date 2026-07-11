use crate::heap::GlobalHeap;

pub const HEAP_START: usize = 0x_4444_4444_0000;
/// End of the lower canonical half on x86_64 (4-level paging).
/// `usize::MAX` is non-canonical and would cause `#GP`, not a page fault.
pub const HEAP_END: usize = 0x0000_7FFF_FFFF_FFFF;
pub const HEAP_SIZE: usize = HEAP_END - HEAP_START + 1;

#[global_allocator]
static HEAP: GlobalHeap = GlobalHeap::new();

pub fn init() {
    HEAP.init(HEAP_START, HEAP_SIZE);
    log::info!("Heap allocator initialized");
}
