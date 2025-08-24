//! x86_64-specific architecture code.
use core::arch::asm;
use x86_64::instructions;
use x86_64::registers::control::{Cr3, Cr3Flags};
pub mod gdt;
pub mod idt;
pub mod paging;

pub type PageTable = paging::PageTable;



/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt()
{
unsafe
{
asm!("hlt");
}
}

/// Initializes x86_64-specific features.
pub fn init()
{
instructions::interrupts::disable();
gdt::init();
idt::init();
// page table is ready. load into Cr3
let root_paddr = crate::memory::PAGE_MAPPER.lock().root_paddr();
let frame = x86_64::structures::paging::PhysFrame::from_start_address(x86_64::PhysAddr::new(root_paddr.as_usize() as u64)).unwrap();

// This is the point of no return. After this instruction, the CPU
// uses our new page table for all memory access.
unsafe { Cr3::write(frame, Cr3Flags::empty()) };
instructions::interrupts::enable();
log::info!("x86_64 architecture initialized.");
}
