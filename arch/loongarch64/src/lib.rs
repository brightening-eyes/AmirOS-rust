//! loongarch64-specific architecture code.
#![no_std]

use core::arch::asm;

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt() {
    unsafe {
        asm!("idle 0");
    }
}

pub mod irqsave;
/// Initializes loongarch64-specific features.
pub fn init() {
    // initialization stuff
    log::info!("loongarch64 architecture initialized.");
}

/// Late platform bring-up. No-op on loongarch64 until interrupt bring-up lands.
pub fn init_platform(_rsdp_paddr: Option<usize>) {}
