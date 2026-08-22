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

pub mod irqsave;
/// Initialize rutines
pub fn init() {
    log::info!("aarch64 architecture initialized.");
}

/// Late platform bring-up. No-op on aarch64 until interrupt bring-up lands.
pub fn init_platform(_rsdp_paddr: Option<usize>) {}
