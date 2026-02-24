use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use free_list::PageLayout;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::Heap as SlabHeap;

const PAGE_SIZE: usize = 4096;

pub struct GlobalHeap {
    heap: UnsafeCell<MaybeUninit<SlabHeap>>,
    initialized: AtomicBool,
}

unsafe impl Send for GlobalHeap {}
unsafe impl Sync for GlobalHeap {}

impl Default for GlobalHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalHeap {
    pub const fn new() -> Self {
        Self {
            heap: UnsafeCell::new(MaybeUninit::uninit()),
            initialized: AtomicBool::new(false),
        }
    }

    pub fn init(&self, heap_start: usize, heap_size: usize) {
        if self.initialized.load(Ordering::Relaxed) {
            return;
        }

        unsafe {
            core::ptr::write(
                self.heap.get(),
                MaybeUninit::new(SlabHeap::new(heap_start, heap_size)),
            );
        }
        self.initialized.store(true, Ordering::Release);
    }

    fn ensure_range_mapped(&self, start: *mut u8, size: usize) -> bool {
        let start_page = (start as usize) & !(PAGE_SIZE - 1);
        let end_page = ((start as usize + size - 1) & !(PAGE_SIZE - 1)) + PAGE_SIZE;

        let mut mapper = match crate::memory::PAGE_MAPPER.try_write() {
            Some(m) => m,
            None => return false,
        };

        let mut frame_alloc = match crate::memory::FRAME_ALLOCATOR.try_write() {
            Some(f) => f,
            None => return false,
        };

        let mut cursor = mapper.cursor();
        let layout = match PageLayout::from_size_align(PAGE_SIZE, PAGE_SIZE) {
            Ok(l) => l,
            Err(_) => return false,
        };

        let mut page = start_page;
        while page < end_page {
            let page_vaddr = VirtAddr::from(page);

            if cursor.query(page_vaddr).is_ok() {
                page += PAGE_SIZE;
                continue;
            }

            let paddr = match frame_alloc.allocate(layout) {
                Ok(range) => PhysAddr::from(range.start()),
                Err(_) => return false,
            };

            if cursor
                .map(
                    page_vaddr,
                    paddr,
                    PageSize::Size4K,
                    MappingFlags::READ | MappingFlags::WRITE,
                )
                .is_err()
            {
                return false;
            }

            page += PAGE_SIZE;
        }

        true
    }
}

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialized.load(Ordering::Relaxed) {
            return core::ptr::null_mut();
        }

        let heap = unsafe { (*self.heap.get()).assume_init_mut() };
        let result = heap.allocate(layout);
        let nptr = match result {
            Ok(p) => p,
            Err(()) => return core::ptr::null_mut(),
        };

        let ptr = nptr.as_ptr();

        if self.ensure_range_mapped(ptr, layout.size()) {
            ptr
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !self.initialized.load(Ordering::Relaxed) || ptr.is_null() {
            return;
        }

        let start_page = (ptr as usize) & !(PAGE_SIZE - 1);
        let end_page = ((ptr as usize + layout.size() - 1) & !(PAGE_SIZE - 1)) + PAGE_SIZE;

        let mut mapper = match crate::memory::PAGE_MAPPER.try_write() {
            Some(m) => m,
            None => return,
        };

        let mut frame_alloc = match crate::memory::FRAME_ALLOCATOR.try_write() {
            Some(f) => f,
            None => return,
        };

        let mut cursor = mapper.cursor();

        let mut page = start_page;
        while page < end_page {
            let page_vaddr = VirtAddr::from(page);

            if let Ok((paddr, _, _)) = cursor.unmap(page_vaddr) {
                let range = paddr.as_usize()..paddr.as_usize() + PAGE_SIZE;
                if let Ok(page_range) = range.try_into() {
                    unsafe {
                        frame_alloc.deallocate(page_range);
                    }
                }
            }

            page += PAGE_SIZE;
        }

        if let Some(nptr) = NonNull::new(ptr) {
            let heap = unsafe { (*self.heap.get()).assume_init_mut() };
            unsafe { heap.deallocate(nptr, layout) };
        }
    }
}
