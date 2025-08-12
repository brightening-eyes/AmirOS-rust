//! Main memory module.
//!
//! This module provides the main interface for memory management, including
//! initializing the frame allocator.

use limine::memory_map::Entry;
use lazy_static::lazy_static;
use spin::Mutex;

pub mod frame;
use frame::BitmapFrameAllocator;

/// The standard page size for our supported architectures, 4 KiB.
pub const PAGE_SIZE: u64 = 4096;

lazy_static! {
    /// The global physical frame allocator.
    ///
    /// This is wrapped in a Mutex to ensure thread-safe access.
    /// We create one instance of the allocator that will live forever.
    pub static ref FRAME_ALLOCATOR: Mutex<BitmapFrameAllocator> =
        Mutex::new(BitmapFrameAllocator::new());
}

/// Initializes the physical frame allocator.
///
/// This function should be called once at boot. It works by locking
/// our global allocator and calling its `init` method.
///
/// # Safety
/// The caller must ensure that the provided memory map is valid and accurately
/// describes the physical memory layout.
pub unsafe fn init(memmap: &[&Entry]) {
    // Correct way: Lock the global instance and call the `init` method on it.
    unsafe {FRAME_ALLOCATOR.lock().init(memmap) };
    log::info!("Physical frame allocator initialized.");
}
