//! Unified, multi-architecture paging using a single handler.

use crate::hhdm_request;
use crate::memory::{FRAME_ALLOCATOR, PAGE_SIZE};
use free_list::PageRange; // Import PageRange for deallocation
use page_table_multiarch::{
    phys_to_virt, PagingHandler,
    // Import the architecture-specific page table types
    x86_64::X64PageTable,
    // We also need the address types from the crate
    PhysAddr, VirtAddr,
};
use spin::Mutex;

// --- OS-Specific Paging Handler ---

#[derive(Clone)]
pub struct OSPagingHandler;

impl PagingHandler for OSPagingHandler {
    fn alloc_frame() -> Option<PhysAddr> {
        // 1. Lock our global allocator.
        let mut allocator = FRAME_ALLOCATOR.lock();
        
        // 2. Request a single page.
        if let Ok(page_range) = allocator.allocate(1) {
            // 3. The PageRange start is a virtual address.
            let vaddr = page_range.start() as usize;

            // 4. We need the HHDM offset to convert it to a physical address.
            let hhdm_offset = HHDM_REQUEST.get_response().unwrap().offset();
            let paddr = vaddr - hhdm_offset;
            
            // 5. Convert the u64 physical address to the PhysAddr type.
            Some(PhysAddr::from(paddr))
        } else {
            // Allocation failed.
            None
        }
    }

    fn dealloc_frame(paddr: PhysAddr) {
        // 1. Get the HHDM offset to convert the physical address to a virtual one.
        let hhdm_offset = HHDM_REQUEST.get_response().unwrap().offset();
        let vaddr_start = paddr.as_usize() + hhdm_offset as usize;
        let vaddr_end = vaddr_start + PAGE_SIZE;

        // 2. Create a PageRange from the virtual address range.
        if let Ok(page_range) = (vaddr_start..vaddr_end).try_into() {
            // 3. Lock the allocator and deallocate the range.
            FRAME_ALLOCATOR.lock().deallocate(page_range);
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        let hhdm_offset = HHDM_REQUEST.get_response().unwrap().offset();
        phys_to_virt(paddr, VirtAddr::from(hhdm_offset as usize))
    }
}
