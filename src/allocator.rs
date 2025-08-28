use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::LockedHeap;
use crate::memory::PAGE_MAPPER;
use crate::memory::FRAME_ALLOCATOR;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024 * 1024; // 100 KiB

pub fn init()
{
let flags = MappingFlags::READ | MappingFlags::WRITE;
for page_offset in (0..HEAP_SIZE).step_by(4096)
{
let vaddr = VirtAddr::from(HEAP_START + page_offset);
let paddr = PhysAddr::from(FRAME_ALLOCATOR.lock().allocate(4096).expect("Failed to allocate a frame for the heap.").start());
PAGE_MAPPER.lock().map(vaddr, paddr, PageSize::Size4K, flags).expect("failed to map the page.").flush();
}
    // Initialize the heap now that the memory is mapped.
unsafe
{
log::info!("initializing heap allocator");
ALLOCATOR.init(HEAP_START, HEAP_SIZE);
}
}
