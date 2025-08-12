//! x86_64-specific architecture code.

use core::arch::asm;

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

/// Initializes x86_64-specific features.
pub fn init() {
    // Later, we will initialize the GDT, IDT, etc. here.
    log::info!("x86_64 architecture initialized.");
}
