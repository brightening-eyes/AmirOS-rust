use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::Heap as SlabHeap;
use spin::Mutex;

use crate::memory::PAGE_SIZE;

fn ensure_range_mapped(start: *mut u8, size: usize) -> bool {
    use free_list::PageLayout;

    let start_page = (start as usize) & !(PAGE_SIZE - 1);
    let end_page = ((start as usize + size - 1) & !(PAGE_SIZE - 1)) + PAGE_SIZE;

    let Ok(layout) = PageLayout::from_size_align(PAGE_SIZE, PAGE_SIZE) else {
        return false;
    };

    let mut page = start_page;
    while page < end_page {
        let page_vaddr = VirtAddr::from(page);

        // Allocate a physical frame. We must do this *before* locking
        // PAGE_MAPPER because cursor.map() may internally lock
        // FRAME_ALLOCATOR (via AmirOSPagingHandler::alloc_frames) to
        // allocate page-table pages.
        let paddr = {
            let mut frame_alloc = crate::memory::FRAME_ALLOCATOR.write();
            match frame_alloc.allocate(layout) {
                Ok(range) => PhysAddr::from(range.start()),
                Err(_) => return false,
            }
        };

        // Map the page under PAGE_MAPPER. If another thread or the page
        // fault handler already mapped this page (race window between the
        // allocation above and here), cursor.map() returns AlreadyMapped
        // which is safe to ignore — we just leak this one frame rather
        // than risk a TOCTOU overwrite.
        {
            let mut mapper = crate::memory::PAGE_MAPPER.write();
            let _ = mapper.cursor().map(
                page_vaddr,
                paddr,
                PageSize::Size4K,
                MappingFlags::READ | MappingFlags::WRITE,
            );
        }

        page += PAGE_SIZE;
    }

    true
}

pub struct GlobalHeap {
    heap: Mutex<Option<SlabHeap>>,
    initialized: AtomicBool,
}

// Safety: GlobalHeap contains a Mutex (which is already Send+Sync) and an
// AtomicBool. All fields are safe to send/share between threads.
unsafe impl Send for GlobalHeap {}
// Safety: same as Send — Mutex provides internal synchronization, AtomicBool
// is natively Sync.
unsafe impl Sync for GlobalHeap {}

impl Default for GlobalHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalHeap {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            heap: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    pub fn init(&self, heap_start: usize, heap_size: usize) {
        if self.initialized.load(Ordering::Relaxed) {
            return;
        }

        // SlabHeap::new() writes intrusive free-list metadata across the slab
        // regions. The heap virtual addresses have no physical backing yet, so
        // each write will trigger a page fault. On x86_64 the page-fault
        // handler lazily allocates a physical frame and maps it on demand.
        // This avoids pre-allocating physical memory for the entire heap
        // (which is 100 MiB) when only a fraction is actually used.
        *self.heap.lock() = unsafe { Some(SlabHeap::new(heap_start, heap_size)) };
        self.initialized.store(true, Ordering::Release);
    }
}

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialized.load(Ordering::Relaxed) || layout.size() == 0 {
            return core::ptr::null_mut();
        }

        if let Some(ref mut heap) = *self.heap.lock() {
            let result = heap.allocate(layout);
            let Ok(nptr) = result else {
                return core::ptr::null_mut();
            };

            let ptr = nptr.as_ptr();

            if ensure_range_mapped(ptr, layout.size()) {
                ptr
            } else {
                core::ptr::null_mut()
            }
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !self.initialized.load(Ordering::Relaxed) || ptr.is_null() || layout.size() == 0 {
            return;
        }

        let start_page = (ptr as usize) & !(PAGE_SIZE - 1);
        let end_page = ((ptr as usize + layout.size() - 1) & !(PAGE_SIZE - 1)) + PAGE_SIZE;

        // Unmap each page and free its physical frame back to the frame
        // allocator. We must not hold PAGE_MAPPER when locking
        // FRAME_ALLOCATOR (cursor.map/unmap may need FRAME_ALLOCATOR
        // internally for page-table page cleanup).
        let mut page = start_page;
        while page < end_page {
            let paddr = {
                let mut mapper = crate::memory::PAGE_MAPPER.write();
                match mapper.cursor().unmap(VirtAddr::from(page)) {
                    Ok((paddr, _, _)) => paddr,
                    Err(_) => {
                        page += PAGE_SIZE;
                        continue;
                    }
                }
            };

            {
                let mut frame_alloc = crate::memory::FRAME_ALLOCATOR.write();
                let start = paddr.as_usize();
                let end = start + PAGE_SIZE;
                if let Ok(page_range) = (start..end).try_into() {
                    frame_alloc.deallocate(page_range);
                }
            }

            page += PAGE_SIZE;
        }

        if let Some(nptr) = NonNull::new(ptr)
            && let Some(ref mut heap) = *self.heap.lock()
        {
            unsafe { heap.deallocate(nptr, layout) };
        }
    }
}
