//! Unified, multi-architecture paging using a single handler.

use crate::HHDM_REQUEST;
use crate::memory::{FRAME_ALLOCATOR, PAGE_SIZE};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{PagingHandler};

#[derive(Clone)]
pub struct AmirOSPagingHandler;

impl PagingHandler for AmirOSPagingHandler {
    fn alloc_frame() -> Option<PhysAddr>
{
let mut allocator = FRAME_ALLOCATOR.lock();        
if let Ok(page_range) = allocator.allocate(1)
{
let vaddr = page_range.start() as usize;
let hhdm_offset = HHDM_REQUEST.get_response().unwrap().offset() as usize;
let paddr = vaddr - hhdm_offset;
Some(PhysAddr::from(paddr))
}
else
{
None
}
}

    fn dealloc_frame(paddr: PhysAddr)
{
let hhdm_offset = HHDM_REQUEST.get_response().unwrap().offset() as usize;
let vaddr_start = paddr.as_usize() + hhdm_offset;
let vaddr_end = vaddr_start + PAGE_SIZE;
if let Ok(page_range) = (vaddr_start..vaddr_end).try_into()
{
unsafe { FRAME_ALLOCATOR.lock().deallocate(page_range) };
}
}

fn phys_to_virt(paddr: PhysAddr) -> VirtAddr
{
VirtAddr::from(paddr.as_usize())
}
}
