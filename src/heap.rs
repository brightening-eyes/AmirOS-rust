use core::alloc::Layout;
use core::alloc::GlobalAlloc;
use spin::RwLock;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::Heap as SlabHeap;

pub struct GlobalHeap {
  heap: RwLock<SlabHeap>,
}

impl GlobalHeap {
  pub fn new(heap_start: usize, heap_size: usize) -> Self {
    let heap = unsafe { RwLock::new(SlabHeap::new(heap_start, heap_size)) };
    Self { heap: heap }
}
}
