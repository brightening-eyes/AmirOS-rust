use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use slab_allocator_rs::{Heap as SlabHeap, HeapAllocator, NUM_OF_SLABS};
use spin::Mutex;

use crate::memory::PAGE_SIZE;

/// Number of pages to grow each slab by on allocation failure.
const GROW_CHUNK: usize = 4 * PAGE_SIZE; // 16 KiB

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
    /// Next virtual address to grow each slab into, indexed by
    /// `HeapAllocator` discriminant (0=64B, 1=128B, ..., 6=4096B, 7=buddy).
    next_addr: Mutex<[usize; NUM_OF_SLABS + 1]>,
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
            next_addr: Mutex::new([0; NUM_OF_SLABS + 1]),
        }
    }

    pub fn init(&self, heap_start: usize, heap_size: usize) {
        if self.initialized.load(Ordering::Relaxed) {
            return;
        }

        let partition_size = heap_size / NUM_OF_SLABS;

        // Initialize next_addr for each slab's partition.
        {
            let mut next = self.next_addr.lock();
            for i in 0..=NUM_OF_SLABS {
                next[i] = heap_start + i * partition_size;
            }
        }

        // SlabHeap::new() writes intrusive free-list metadata across the slab
        // regions. The heap virtual addresses have no physical backing yet, so
        // each write will trigger a page fault. On x86_64 the page-fault
        // handler lazily allocates a physical frame and maps it on demand.
        *self.heap.lock() = unsafe { Some(SlabHeap::new(heap_start, heap_size)) };
        self.initialized.store(true, Ordering::Release);
    }

    /// Grow a slab by `GROW_CHUNK` pages when `allocate()` fails.
    /// Returns `true` if the grow succeeded and the caller should retry.
    fn grow_heap(&self, allocator: HeapAllocator) -> bool {
        let idx = allocator as usize;
        let partition_size = crate::allocator::HEAP_SIZE / NUM_OF_SLABS;
        let partition_end = crate::allocator::HEAP_START + (idx + 1) * partition_size;

        // Pick the next address and advance it, but don't exceed the partition.
        let addr = {
            let mut next = self.next_addr.lock();
            let a = next[idx];
            if a + GROW_CHUNK > partition_end {
                return false;
            }
            next[idx] = a + GROW_CHUNK;
            a
        };

        // grow() writes to the new memory region to initialize free-list
        // metadata. These writes are serviced by the page-fault handler
        // which maps physical frames on demand.
        unsafe {
            if let Some(ref mut heap) = *self.heap.lock() {
                heap.grow(addr, GROW_CHUNK, allocator);
            } else {
                return false;
            }
        }

        true
    }
}

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialized.load(Ordering::Relaxed) || layout.size() == 0 {
            return core::ptr::null_mut();
        }

        if let Some(ref mut heap) = *self.heap.lock() {
            match heap.allocate(layout) {
                Ok(nptr) => {
                    let ptr = nptr.as_ptr();
                    if ensure_range_mapped(ptr, layout.size()) {
                        ptr
                    } else {
                        core::ptr::null_mut()
                    }
                }
                Err(()) => {
                    // Slab is full — grow it by GROW_CHUNK and retry once.
                    let allocator = SlabHeap::layout_to_allocator(&layout);
                    let _ = heap;
                    if self.grow_heap(allocator) {
                        return unsafe { self.alloc(layout) };
                    }
                    core::ptr::null_mut()
                }
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
