//! x86_64-specific architecture code.
use core::arch::asm;
use x86_64::instructions;
pub mod idt;
pub mod paging;

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt() -> !
{
loop
{
unsafe
{
asm!("hlt");
}
}
}

/// Initializes x86_64-specific features.
pub fn init()
{
instructions::interrupts::disable();
idt::init();
paging::init();
instructions::interrupts::enable();
log::info!("x86_64 architecture initialized.");
}
