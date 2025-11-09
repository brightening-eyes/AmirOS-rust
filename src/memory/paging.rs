//! Unified, multi-architecture paging using a single handler.
use crate::memory::{FRAME_ALLOCATOR, PAGE_SIZE};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::PagingHandler;

#[derive(Clone)]
pub struct AmirOSPagingHandler;

impl PagingHandler for AmirOSPagingHandler {
    fn alloc_frame() -> Option<PhysAddr> {
        match FRAME_ALLOCATOR.try_write() {
            Some(mut allocator) => {
                if let Ok(page_range) = allocator.allocate(PAGE_SIZE) {
                    let paddr = page_range.start();
                    Some(PhysAddr::from(paddr))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn dealloc_frame(paddr: PhysAddr) {
        match FRAME_ALLOCATOR.try_write() {
            Some(mut allocator) => {
                let vaddr_start = paddr.as_usize();
                let vaddr_end = vaddr_start + PAGE_SIZE;
                if let Ok(page_range) = (vaddr_start..vaddr_end).try_into() {
                    unsafe { allocator.deallocate(page_range) };
                }
            }
            None => (),
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        let pa = paddr.as_usize();
        pa.checked_add(FRAME_ALLOCATOR.read().hhdm_offset)
            .map(VirtAddr::from_usize)
            .expect("failed to allocate address")
    }
}
