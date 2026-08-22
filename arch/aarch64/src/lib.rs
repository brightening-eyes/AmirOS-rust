//! aarch64-specific architecture code.
#![no_std]

use core::arch::asm;

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
