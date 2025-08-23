//! loongarch64-specific architecture code.

use core::arch::asm;

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt()
{
unsafe
{
asm!("idle 0");
}
}

/// Initializes loongarch64-specific features.
pub fn init()
{
// initialization stuff
    log::info!("loongarch64 architecture initialized.");
}
