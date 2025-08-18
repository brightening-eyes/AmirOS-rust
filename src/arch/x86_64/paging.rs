//! x86_64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use lazy_static::lazy_static;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{x86_64::X64PageTable, MappingFlags, PageSize};
use x86_64::registers::control::{Cr3, Cr3Flags};
use spin::Mutex;
use limine::memory_map::EntryType;

/// A type alias for the x86_64-specific page table, using our OS's handler.
pub type PageTable = X64PageTable<AmirOSPagingHandler>;

lazy_static!
{
pub static ref PAGE_MAPPER: Mutex<PageTable> = {
let page_table = PageTable::try_new().expect("Failed to create x86_64 page table");
Mutex::new(page_table)
    };
}

/// Initializes and activates the x86_64 page table.
///
/// This function is called from the main architecture initialization routine.
pub fn init()
{    
// Get the necessary information from the bootloader.
let hhdm_offset = crate::HHDM_REQUEST.get_response().unwrap().offset();
let kernel_addr = crate::EXECUTABLE_ADDRESS_REQUEST.get_response().unwrap();
let memmap = crate::MEMORY_MAP_REQUEST.get_response().unwrap();
let mut mapper = PAGE_MAPPER.lock();
// 1. Minimal first mapping: map first 2 MiB (identity map)
//    This ensures the CPU can safely execute after loading CR3
// ------------------------------------------------------------
let first_page_flags = MappingFlags::READ | MappingFlags::WRITE;
let minimal_pa = PhysAddr::from(0);
let minimal_va = VirtAddr::from(0);
let _ = mapper.map(minimal_va, minimal_pa, PageSize::Size2M, first_page_flags).expect("Failed to map minimal first page");
let root_paddr = mapper.root_paddr();
let frame = x86_64::structures::paging::PhysFrame::from_start_address(x86_64::PhysAddr::new(root_paddr.as_usize() as u64)).unwrap();
unsafe { Cr3::write(frame, Cr3Flags::empty()) };
// --- 1. Map all of physical memory to the HHDM offset ---
let flags = MappingFlags::READ | MappingFlags::WRITE;
for entry in memmap.entries()
{
if !matches!(entry.entry_type, EntryType::USABLE)
{
continue
}
let start_pa = entry.base as u64;
let end_pa   = start_pa + entry.length as u64;
// align down/up to 4K
let start_pa_aligned = start_pa & !((crate::memory::PAGE_SIZE as u64) - 1);
let end_pa_aligned   = (end_pa + (crate::memory::PAGE_SIZE as u64) - 1) & !((crate::memory::PAGE_SIZE as u64) - 1);
let mut pa = start_pa_aligned;
while pa < end_pa_aligned
{
let va = pa + hhdm_offset;
let paddr = PhysAddr::from(pa as usize);
let vaddr = VirtAddr::from(va as usize);
let _ = mapper.map(vaddr, paddr, PageSize::Size4K, flags).expect("Failed to map HHDM page");
pa += crate::memory::PAGE_SIZE as u64;
        }
log::info!("mapped from {} to {}", start_pa_aligned, end_pa_aligned);
    }
    log::info!("physical memory mapped.");
// --- 2. Map the kernel's own sections ---
// (This is often covered by the HHDM, but explicit mapping is safer)
/*
let kernel_paddr = PhysAddr::from(kernel_addr.physical_base() as usize);
let kernel_vaddr = VirtAddr::from(kernel_addr.virtual_base() as usize);
let kernel_size = 16 * 1024 * 1024;
let kflags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;
let mut off = 0usize;
while off < kernel_size
{
let paddr = kernel_paddr + off;
let vaddr = kernel_vaddr + off;
let _ = mapper.map(vaddr, paddr, PageSize::Size4K, kflags).expect("Failed to map kernel page");
off += crate::memory::PAGE_SIZE;
}
*/
log::info!("x86_64 paging initialized and activated.");
}
