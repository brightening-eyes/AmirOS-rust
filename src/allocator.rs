//! A minimal example that implements the GlobalAlloc trait.

use core::alloc::{GlobalAlloc, Layout};
use core::mem::transmute;
use core::ptr::{self, NonNull};
use slabmalloc::*;
use spin::Mutex;
use crate::arch::arch::paging::PAGE_MAPPER;

/// SLAB_ALLOC is set as the system's default allocator, it's implementation follows below.
///
/// It's a ZoneAllocator wrapped inside a Mutex.
#[global_allocator]
static SLAB_ALLOC: SafeZoneAllocator = SafeZoneAllocator(Mutex::new(ZoneAllocator::new()));

struct KernelPager;

impl KernelPager
{
const BASE_PAGE_SIZE: usize = 4096;
const LARGE_PAGE_SIZE: usize = 2 * 1024 * 1024;

/// Allocates a given `page_size`.
fn alloc_page(&mut self, page_size: usize) -> Option<*mut u8>
{
let r = unsafe { System.alloc(Layout::from_size_align(page_size, page_size).unwrap()) };

if !r.is_null()
{
Some(r)
}
else
{
None
}
}

/// De-allocates a given `page_size`.
fn dealloc_page(&mut self, ptr: *mut u8, page_size: usize)
{
let layout = match page_size
{
KernelPager::BASE_PAGE_SIZE =>
{
Layout::from_size_align(KernelPager::BASE_PAGE_SIZE, KernelPager::BASE_PAGE_SIZE).unwrap()
}
KernelPager::LARGE_PAGE_SIZE =>
{
Layout::from_size_align(KernelPager::LARGE_PAGE_SIZE, KernelPager::LARGE_PAGE_SIZE).unwrap()
}
_ => unreachable!("invalid page-size supplied"),
};

unsafe { System.dealloc(ptr, layout) };
}

/// Allocates a new ObjectPage from the System.
fn allocate_page(&mut self) -> Option<&'static mut ObjectPage<'static>>
{
self.alloc_page(KernelPager::BASE_PAGE_SIZE).map(|r| unsafe { transmute(r as usize) })
}

/// Release a ObjectPage back to the System.
#[allow(unused)]
fn release_page(&mut self, p: &'static mut ObjectPage<'static>)
{
self.dealloc_page(p as *const ObjectPage as *mut u8, KernelPager::BASE_PAGE_SIZE);
}

/// Allocates a new LargeObjectPage from the system.
fn allocate_large_page(&mut self) -> Option<&'static mut LargeObjectPage<'static>>
{
self.alloc_page(KernelPager::LARGE_PAGE_SIZE).map(|r| unsafe { transmute(r as usize) })
}

/// Release a LargeObjectPage back to the System.
#[allow(unused)]
fn release_large_page(&mut self, p: &'static mut LargeObjectPage<'static>)
{
self.dealloc_page(p as *const LargeObjectPage as *mut u8, KernelPager::LARGE_PAGE_SIZE);
}
}

/// A pager for GlobalAlloc.
static mut KernelPAGER: KernelPager = KernelPager;

/// A SafeZoneAllocator that wraps the ZoneAllocator in a Mutex.
///
/// Note: This is not very scalable since we use a single big lock
/// around the allocator. There are better ways make the ZoneAllocator
/// thread-safe directly, but they are not implemented yet.
pub struct SafeZoneAllocator(Mutex<ZoneAllocator<'static>>);

unsafe impl GlobalAlloc for SafeZoneAllocator {
unsafe fn alloc(&self, layout: Layout) -> *mut u8
{
match layout.size()
{
KernelPager::BASE_PAGE_SIZE =>
{
// Best to use the underlying backend directly to allocate pages
// to avoid fragmentation
unsafe {KernelPAGER.allocate_page().expect("Can't allocate page?") as *mut _ as *mut u8}
}
KernelPager::LARGE_PAGE_SIZE =>
{
// Best to use the underlying backend directly to allocate large
// to avoid fragmentation
unsafe {KernelPAGER.allocate_large_page().expect("Can't allocate page?") as *mut _ as *mut u8}
}
0..=ZoneAllocator::MAX_ALLOC_SIZE =>
{
let mut zone_allocator = self.0.lock();
match zone_allocator.allocate(layout)
{
Ok(nptr) => nptr.as_ptr(),
Err(AllocationError::OutOfMemory) =>
{
if layout.size() <= ZoneAllocator::MAX_BASE_ALLOC_SIZE
{
unsafe {KernelPAGER.allocate_page().map_or(ptr::null_mut(), |page|
{
zone_allocator.refill(layout, page).expect("Could not refill?");
zone_allocator.allocate(layout).expect("Should succeed after refill").as_ptr()
})}
}
else
{
// layout.size() <= ZoneAllocator::MAX_ALLOC_SIZE
unsafe {KernelPAGER.allocate_large_page().map_or(ptr::null_mut(), |large_page| {
zone_allocator.refill_large(layout, large_page).expect("Could not refill?");
zone_allocator.allocate(layout).expect("Should succeed after refill").as_ptr()
})}
}
}
Err(AllocationError::InvalidLayout) => panic!("Can't allocate this size"),
}
}
_ => unimplemented!("Can't handle it, probably needs another allocator."),
}
}

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout)
{
match layout.size()
{
KernelPager::BASE_PAGE_SIZE => KernelPager.dealloc_page(ptr, KernelPager::BASE_PAGE_SIZE),
KernelPager::LARGE_PAGE_SIZE => KernelPager.dealloc_page(ptr, KernelPager::LARGE_PAGE_SIZE),
0..=ZoneAllocator::MAX_ALLOC_SIZE =>
{
if let Some(nptr) = NonNull::new(ptr)
{
self.0.lock().deallocate(nptr, layout).expect("Couldn't deallocate");
}

// An proper reclamation strategy could be implemented here
// to release empty pages back from the ZoneAllocator to the PAGER
}
_ => unimplemented!("Can't handle it, probably needs another allocator."),
}
}
}
