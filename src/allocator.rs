use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::LockedHeap;
use crate::arch::arch::paging::PAGE_MAPPER;
use crate::memory::FRAME_ALLOCATOR;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init()
{
let heap_start = 0;
let heap_end = 10*1024*1024;
let heap_size = heap_end - heap_start;
let flags = MappingFlags::READ | MappingFlags::WRITE;
let _heap = (heap_start..heap_size).step_by(4096).map(|r| {
PAGE_MAPPER.lock().map(VirtAddr::from(r as usize), PhysAddr::from(FRAME_ALLOCATOR.lock().allocate(4096).unwrap().start()), PageSize::Size4K, flags).expect("failed to map the page.").flush()
});
// we have mapped the page, lets initialize the heap
unsafe
{
ALLOCATOR.init(heap_start, heap_size);
}
}
