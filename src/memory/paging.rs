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
        let size = num_pages
            .checked_mul(0x1000)
            .expect("paging: integer overflow in alloc_frames size");
        let layout: PageLayout = PageLayout::from_size_align(size, align)
            .expect("paging: invalid page layout for alloc_frames");
        let mut allocator = FRAME_ALLOCATOR.write();
        if let Ok(page_range) = allocator.allocate(layout) {
            let paddr = page_range.start();
            Some(PhysAddr::from(paddr))
        } else {
            None
        }
    }

    fn dealloc_frames(paddr: PhysAddr, num_pages: usize) {
        let size = num_pages
            .checked_mul(0x1000)
            .expect("paging: integer overflow in dealloc_frames size");
        let layout = Layout::from_size_align(size, 0x1000)
            .expect("paging: invalid layout for dealloc_frames");
        let mut allocator = FRAME_ALLOCATOR.write();
        let vaddr_start = paddr.as_usize();
        let vaddr_end = vaddr_start
            .checked_add(layout.size())
            .expect("paging: integer overflow in dealloc_frames range");
        if let Ok(page_range) = (vaddr_start..vaddr_end).try_into() {
            allocator.deallocate(page_range);
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        let pa = paddr.as_usize();
        pa.checked_add(FRAME_ALLOCATOR.read().hhdm_offset)
            .map(VirtAddr::from_usize)
            .expect("failed to allocate address")
    }
}
