//! Unified, multi-architecture paging using a single handler.
use crate::memory::{FRAME_ALLOCATOR, PAGE_SIZE};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{PagingHandler};
use core::sync::atomic::{AtomicUsize, Ordering};

static HHDM_OFFSET: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct AmirOSPagingHandler;

pub fn init_hhdm_offset()
{
let offset = FRAME_ALLOCATOR.lock().hhdm_offset as usize;
HHDM_OFFSET.store(offset, Ordering::Relaxed);
}

impl PagingHandler for AmirOSPagingHandler {
    fn alloc_frame() -> Option<PhysAddr>
{
let mut allocator = FRAME_ALLOCATOR.lock();        
if let Ok(page_range) = allocator.allocate(PAGE_SIZE)
{
let paddr = page_range.start() as usize;
Some(PhysAddr::from(paddr as usize))
}
else
{
None
}
}

    fn dealloc_frame(paddr: PhysAddr)
{
let vaddr_start = paddr.as_usize();
let vaddr_end = vaddr_start + PAGE_SIZE;
if let Ok(page_range) = (vaddr_start..vaddr_end).try_into()
{
unsafe { FRAME_ALLOCATOR.lock().deallocate(page_range) };
}
}

fn phys_to_virt(paddr: PhysAddr) -> VirtAddr
{
let offset = HHDM_OFFSET.load(Ordering::Relaxed);
assert!(offset != 0, "HHDM offset not initialized");
let pa = paddr.as_usize();
offset.checked_add(pa).map(VirtAddr::from_usize).expect("failed to allocate address")
}

}
