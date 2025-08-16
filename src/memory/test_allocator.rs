//! A freelist-based physical frame allocator.

use limine::memory_map::{Entry, EntryType};

/// Represents a node in the freelist. Each free frame of memory
/// will be treated as one of these nodes.
#[repr(C)]
struct Node {
    next: Option<&'static mut Node>,
}

/// A physical frame allocator that uses a freelist.
pub struct FrameAllocator {
    head: Option<&'static mut Node>,
    hhdm_offset: u64,
    page_size: usize,
}

// The allocator contains raw pointers, but we know access is safe
// because it will be protected by a Mutex.
unsafe impl Send for FrameAllocator {}

impl FrameAllocator {
    /// Creates a new, empty allocator. The HHDM offset and page size
    /// for the current architecture must be provided.
    pub const fn new(hhdm_offset: u64, page_size: usize) -> Self {
        Self { head: None, hhdm_offset, page_size }
    }

    /// Initializes the allocator with the memory map from the bootloader.
    ///
    /// # Safety
    /// This function is unsafe because it relies on a valid memory map and
    /// writes directly to physical memory to build the freelist.
    pub unsafe fn init(&mut self, memmap: &[&Entry]) {
        // copy page size:
        let page_size = self.page_size;
        // Create an iterator of the start addresses of all usable frames.
        let usable_frames = memmap.iter()
            // 1. Filter for regions that are usable.
            .filter(|entry| entry.entry_type == EntryType::USABLE)
            // 2. Map each region to its address range.
            .map(|entry| entry.base..(entry.base + entry.length))
            // 3. Flat-map the ranges into an iterator of frame-aligned addresses.
            .flat_map(|range| range.step_by(page_size));

        // Iterate over all usable frame addresses and add them to our list.
        for frame_addr in usable_frames {
            unsafe { self.deallocate(frame_addr) };
        }

        let free_frames = self.count_free_frames();
        log::info!("Freelist allocator initialized with {} free frames.", free_frames);
    }

    /// Allocates a single physical frame, returning its starting address.
    pub fn allocate(&mut self) -> Option<u64> {
        if let Some(head_node) = self.head.take() {
            self.head = head_node.next.take();
            let vaddr = head_node as *const Node as u64;
            // Convert the virtual address back to a physical one.
            Some(vaddr - self.hhdm_offset)
        } else {
            None
        }
    }

    /// Deallocates a frame, adding it to the head of the freelist.
    ///
    /// # Safety
    /// The caller must ensure the provided physical address is valid and unused.
    pub unsafe fn deallocate(&mut self, paddr: u64) {
        // Convert the physical address to a virtual one so we can write to it.
        let vaddr = paddr + self.hhdm_offset;
        let new_node = unsafe { &mut *(vaddr as *mut Node) };

        // Prepend the new node to the list.
        new_node.next = self.head.take();
        self.head = Some(new_node);
    }

    /// Helper function to count frames for logging.
    fn count_free_frames(&self) -> u64 {
        let mut count = 0;
        let mut current = self.head.as_ref();
        while let Some(node) = current {
            count += 1;
            current = node.next.as_ref();
        }
        count
    }
}
