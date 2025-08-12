//! A bitmap-based physical frame allocator.

// CORRECTED: Use the new paths for the memory map types.
use limine::memory_map::{Entry, EntryType};
use crate::memory::PAGE_SIZE;

/// A physical frame allocator that uses a bitmap to track free frames.
pub struct BitmapFrameAllocator {
    bitmap: *mut u8,
    total_frames: u64,
    free_frames: u64,
    next_free_idx: u64,
}

// By implementing `Send`, we promise the compiler that it is safe to move this
// struct between threads. This is safe because all access to the raw pointer
// `bitmap` is protected by a Mutex.
unsafe impl Send for BitmapFrameAllocator {}

impl BitmapFrameAllocator {
    /// Creates a new, uninitialized frame allocator.
    pub const fn new() -> Self {
        BitmapFrameAllocator {
            bitmap: core::ptr::null_mut(),
            total_frames: 0,
            free_frames: 0,
            next_free_idx: 0,
        }
    }

    /// Initializes the allocator with the given memory map.
    ///
    /// # Safety
    /// This function is unsafe because it writes to raw memory based on the
    /// provided memory map. The caller must ensure the map is valid.
    // CORRECTED: The type for the memory map is now `&[&Entry]`.
    pub unsafe fn init(&mut self, memmap: &[&Entry]) {
        // First, find the total amount of memory and the highest address.
        let (_total_memory, highest_addr) = memmap
            .iter()
            .fold((0, 0), |(total, high), entry| {
                (total + entry.length, high.max(entry.base + entry.length))
            });

        self.total_frames = highest_addr / PAGE_SIZE;
        let bitmap_size = self.total_frames / 8;

        // Find a large enough region of usable memory to place our bitmap.
        let bitmap_location = memmap
            .iter()
            .find(|entry| {
                // CORRECTED: Use the new enum variant `Type::USABLE`.
                entry.entry_type == EntryType::USABLE && entry.length >= bitmap_size
            })
            .map(|entry| entry.base)
            .expect("No suitable memory region found for frame allocator bitmap");

        self.bitmap = bitmap_location as *mut u8;

        // Mark all memory as used by default.
        unsafe { core::ptr::write_bytes(self.bitmap, 0xFF, bitmap_size as usize) };

        // Now, iterate through the memory map again and mark usable frames as free (0).
        for entry in memmap {
            // CORRECTED: Use the new enum variant `Type::USABLE`.
            if entry.entry_type == EntryType::USABLE {
                // Align the start up and the end down to the nearest page boundary.
                let start_frame = (entry.base + PAGE_SIZE - 1) / PAGE_SIZE;
                let end_frame = (entry.base + entry.length) / PAGE_SIZE;

                for frame in start_frame..end_frame {
                    self.clear_bit(frame);
                    self.free_frames += 1;
                }
            }
        }

        // Finally, mark the bitmap's own frames as used.
        let bitmap_start_frame = bitmap_location / PAGE_SIZE;
        let bitmap_end_frame = (bitmap_location + bitmap_size) / PAGE_SIZE;
        for frame in bitmap_start_frame..=bitmap_end_frame {
            // Only decrement free_frames if the frame was actually free before.
            if !self.get_bit(frame) {
                 self.set_bit(frame);
                 self.free_frames -= 1;
            }
        }
    }

    /// Allocates a single physical frame, returning its starting address.
    pub fn allocate(&mut self) -> Option<u64> {
        if self.free_frames == 0 {
            return None;
        }

        // Naive linear scan for a free frame.
        // TODO: Improve this with a more efficient search.
        for i in self.next_free_idx..self.total_frames {
            if !self.get_bit(i) {
                self.set_bit(i);
                self.free_frames -= 1;
                self.next_free_idx = i + 1;
                return Some(i * PAGE_SIZE);
            }
        }
        // If we didn't find one, wrap around.
        for i in 0..self.next_free_idx {
             if !self.get_bit(i) {
                self.set_bit(i);
                self.free_frames -= 1;
                self.next_free_idx = i + 1;
                return Some(i * PAGE_SIZE);
            }
        }

        None // Should be unreachable if free_frames > 0
    }

    // --- Bitmap helper functions ---
    fn get_bit(&self, index: u64) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;
        unsafe { ((*self.bitmap.add(byte_index as usize)) >> bit_index) & 1 != 0 }
    }

    fn set_bit(&mut self, index: u64) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        unsafe {
            *self.bitmap.add(byte_index as usize) |= 1 << bit_index;
        }
    }
    
    fn clear_bit(&mut self, index: u64) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        unsafe {
            *self.bitmap.add(byte_index as usize) &= !(1 << bit_index);
        }
    }
}
