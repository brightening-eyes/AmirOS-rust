//! riscv64-specific architecture code.

use core::arch::asm;
use riscv::register::satp;
pub mod paging;

pub type PageTable = paging::PageTable;
pub type PageTableEntry = paging::PageTableEntry;

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt() {
    unsafe {
        asm!("wfi");
    }
}

/// Initializes riscv64-specific features.
pub fn init() {
    match crate::memory::PAGE_MAPPER.try_read() {
        Some(mapper) => {
            let root_paddr = mapper.root_paddr().as_usize();
            let ppn = root_paddr / 4096; // Convert address to Physical Page Number
            unsafe { satp::set(satp::Mode::Sv48, 0, ppn) };
        }
        None => {
            panic!("error reading page map!.");
        }
    }
    log::info!("riscv64 architecture initialized.");
}
