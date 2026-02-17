//! Unified, multi-architecture paging using a single handler.
use crate::memory::FRAME_ALLOCATOR;
use core::alloc::Layout;
use free_list::PageLayout;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::PagingHandler;

#[derive(Clone)]
pub struct AmirOSPagingHandler;

impl PagingHandler for AmirOSPagingHandler {
    fn alloc_frames(num_pages: usize, align: usize) -> Option<PhysAddr> {
        let layout: PageLayout = PageLayout::from_size_align(num_pages * 0x1000, align).unwrap();
        match FRAME_ALLOCATOR.try_write() {
            Some(mut allocator) => {
                if let Ok(page_range) = allocator.allocate(layout) {
                    let paddr = page_range.start();
                    Some(PhysAddr::from(paddr))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn dealloc_frames(paddr: PhysAddr, num_pages: usize) {
        let layout = Layout::from_size_align(num_pages * 0x1000, 0x1000).unwrap();
        if let Some(mut allocator) = FRAME_ALLOCATOR.try_write() {
            let vaddr_start = paddr.as_usize();
            let vaddr_end = vaddr_start + layout.size();
            if let Ok(page_range) = (vaddr_start..vaddr_end).try_into() {
                unsafe { allocator.deallocate(page_range) };
            }
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        let pa = paddr.as_usize();
        pa.checked_add(FRAME_ALLOCATOR.read().hhdm_offset)
            .map(VirtAddr::from_usize)
            .expect("failed to allocate address")
    }
}
