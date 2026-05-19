//! riscv64-specific architecture code.

use core::arch::asm;
use memory_addr::VirtAddr;
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
    let mapper = crate::memory::PAGE_MAPPER.read();
    let root_paddr = mapper.root_paddr().as_usize();
    let ppn = root_paddr / 4096; // Convert address to Physical Page Number
    unsafe { satp::set(satp::Mode::Sv48, 0, ppn) };
    drop(mapper);

    // Setup guard page below kernel stack.
    let sp: usize;
    unsafe { asm!("mv {}, sp", out(reg) sp, options(nomem, nostack)) };
    let stack_top = (sp + 0xFFF) & !0xFFF;
    let stack_bottom = stack_top.saturating_sub(128 * 1024);
    let guard_page = stack_bottom.saturating_sub(0x1000);
    let mut mapper = crate::memory::PAGE_MAPPER.write();
    let _ = mapper.cursor().unmap(VirtAddr::from(guard_page));

    log::info!("riscv64 architecture initialized.");
}
