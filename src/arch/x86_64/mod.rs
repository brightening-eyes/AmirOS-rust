//! x86_64-specific architecture code.
use core::arch::asm;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use x86_64::instructions;
use x86_64::registers::control::{Cr3, Cr3Flags};
pub mod gdt;
pub mod idt;
pub mod paging;

pub type PageTable = paging::PageTable;
pub type PageTableEntry = paging::PageTableEntry;

/// Walk the currently active 4-level page tables to translate a virtual
/// address to its physical address. Used before the CR3 switch to find
/// the physical frames backing the Limine-provided stack.
fn virt_to_phys(virt: usize) -> Option<usize> {
    let cr3: usize;
    unsafe { asm!("mov {}, cr3", out(reg) cr3) };
    let hhdm = crate::memory::FRAME_ALLOCATOR.read().hhdm_offset;

    let pml4 = unsafe { &*((cr3 + hhdm) as *const [u64; 512]) };
    let pml4e = pml4[(virt >> 39) & 0x1FF];
    if pml4e & 1 == 0 {
        return None;
    }
    if pml4e & (1 << 7) != 0 {
        return Some((pml4e as usize & 0x000FFFFFC0000000) | (virt & 0x3FFFFFFF));
    }

    let pdpt = unsafe { &*(((pml4e as usize & 0x000FFFFFFFFFF000) + hhdm) as *const [u64; 512]) };
    let pdpte = pdpt[(virt >> 30) & 0x1FF];
    if pdpte & 1 == 0 {
        return None;
    }
    if pdpte & (1 << 7) != 0 {
        return Some((pdpte as usize & 0x000FFFFFC0000000) | (virt & 0x3FFFFFFF));
    }

    let pd = unsafe { &*(((pdpte as usize & 0x000FFFFFFFFFF000) + hhdm) as *const [u64; 512]) };
    let pde = pd[(virt >> 21) & 0x1FF];
    if pde & 1 == 0 {
        return None;
    }
    if pde & (1 << 7) != 0 {
        return Some((pde as usize & 0x000FFFFFFFE00000) | (virt & 0x1FFFFF));
    }

    let pt = unsafe { &*(((pde as usize & 0x000FFFFFFFFFF000) + hhdm) as *const [u64; 512]) };
    let pte = pt[(virt >> 12) & 0x1FF];
    if pte & 1 == 0 {
        return None;
    }

    Some((pte as usize & 0x000FFFFFFFFFF000) | (virt & 0xFFF))
}

/// Halts the CPU.
///
/// This function enters an infinite loop and uses the `hlt` instruction
/// to put the CPU into a low-power state until the next interrupt.
pub fn holt() {
    unsafe {
        asm!("hlt");
    }
}

/// Initialization code for `x86_64`.
/// this function performs the initialization code for the processor.
/// # Panics
/// when initialization fails, we will panic here as the continuation of everything is impossible.
pub fn init() {
    instructions::interrupts::disable();
    gdt::init();
    idt::init();

    // Map the Limine-provided stack pages into our new page table so the
    // stack remains accessible after the CR3 switch.
    let rsp: usize;
    unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    let stack_top = (rsp + 0xFFF) & !0xFFF;
    let stack_base = stack_top.saturating_sub(128 * 1024);
    let flags = MappingFlags::READ | MappingFlags::WRITE;
    {
        let mut mapper = crate::memory::PAGE_MAPPER.write();
        let mut addr = stack_base;
        while addr < stack_top {
            if let Some(phys) = virt_to_phys(addr) {
                let vaddr = VirtAddr::from(addr);
                let paddr = PhysAddr::from(phys);
                let _ = mapper.cursor().map(vaddr, paddr, PageSize::Size4K, flags);
            }
            addr += crate::memory::PAGE_SIZE;
        }
    }

    // page table is ready. load into Cr3
    let mapper = crate::memory::PAGE_MAPPER.read();
    let root_paddr = mapper.root_paddr();
    let frame = x86_64::structures::paging::PhysFrame::from_start_address(x86_64::PhysAddr::new(
        root_paddr.as_usize() as u64,
    ))
    .expect("x86_64: could not load the memory into provided paging structure.");
    // This is the point of no return. After this instruction, the CPU
    // uses our new page table for all memory access.
    unsafe { Cr3::write(frame, Cr3Flags::empty()) };
    drop(mapper);
    instructions::interrupts::enable();

    // Set up a guard page below the kernel stack to catch stack overflows.
    // Read the current stack pointer and unmap the page immediately below
    // the stack's estimated bottom (128 KiB below the top).
    let rsp: usize;
    unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    let stack_top = (rsp + 0xFFF) & !0xFFF;
    let stack_bottom = stack_top.saturating_sub(128 * 1024);
    let guard_page = stack_bottom.saturating_sub(0x1000);
    let mut mapper = crate::memory::PAGE_MAPPER.write();
    let _ = mapper.cursor().unmap(VirtAddr::from(guard_page));

    log::info!("x86_64 architecture initialized.");
}
