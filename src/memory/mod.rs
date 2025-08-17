// memory management
use limine::memory_map::Entry;
use lazy_static::lazy_static;
use spin::Mutex;
pub mod allocator;
pub mod paging;

pub static PAGE_SIZE: usize = 4096;

lazy_static!
{
pub static ref FRAME_ALLOCATOR: Mutex<allocator::FrameAllocator> = {
let hhdm_offset = crate::HHDM_REQUEST.get_response().unwrap().offset();
Mutex::new(allocator::FrameAllocator::new(hhdm_offset))
};
}

pub fn init(memmap: &[&Entry])
{
FRAME_ALLOCATOR.lock().init(memmap);
}
