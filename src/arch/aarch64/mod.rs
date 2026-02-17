//! aarch64-specific architecture code.

use core::arch::asm;
pub mod paging;

pub type PageTable = paging::PageTable;
pub type PageTableEntry = paging::PageTableEntry;

/// Halts the CPU.
pub fn holt() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

/// Initialize rutines
pub fn init() {
    log::info!("aarch64 architecture initialized.");
}
