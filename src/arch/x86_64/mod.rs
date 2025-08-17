//! x86_64-specific architecture code.
use core::arch::asm;
mod idt;
mod paging;

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
idt::init();
paging::init();
    log::info!("x86_64 architecture initialized.");
}
