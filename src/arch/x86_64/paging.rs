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

const PAGE_SIZE_1G: u64 = 1024 * 1024 * 1024;
const PAGE_SIZE_2M: u64 = 2 * 1024 * 1024;

/// Initializes and activates the x86_64 page table.
///
/// This function is called from the main architecture initialization routine.
pub fn init()
{    
// Get the necessary information from the bootloader.
let hhdm_offset = crate::HHDM_REQUEST.get_response().unwrap().offset();
let kernel_addr = crate::EXECUTABLE_ADDRESS_REQUEST.get_response().unwrap();
let kernel_file = crate::EXECUTABLE_FILE_REQUEST.get_response().unwrap().file();
let memmap = crate::MEMORY_MAP_REQUEST.get_response().unwrap();
let mut mapper = PAGE_MAPPER.lock();
let flags = MappingFlags::READ | MappingFlags::WRITE;

// First, map all physical memory to the higher-half direct map (HHDM) region.
// We also identity-map the first 4GiB. This is a robust technique to ensure
// that the CPU can continue execution seamlessly after the CR3 switch, as it
// makes physical addresses temporarily valid as virtual addresses.
for entry in memmap.entries()
{
// We map all memory types except for bad memory. This includes the kernel,
// modules, and bootloader-reclaimable memory.
if matches!(entry.entry_type, EntryType::BAD_MEMORY)
{
continue;
}
let start_pa = entry.base;
let end_pa = start_pa + entry.length;
let mut pa = start_pa;

while pa < end_pa
{
let remaining = end_pa - pa;
let paddr = PhysAddr::from(pa as usize);

// Prioritize the largest possible page size.
if pa % PAGE_SIZE_1G == 0 && (pa + hhdm_offset) % PAGE_SIZE_1G == 0 && remaining >= PAGE_SIZE_1G
{
let vaddr = VirtAddr::from((pa + hhdm_offset) as usize);
mapper.map(vaddr, paddr, PageSize::Size1G, flags).expect("Failed to map 1G HHDM page").flush();
if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa as usize);
mapper.map(identity_vaddr, paddr, PageSize::Size1G, flags).expect("Failed to identity map 1G low page").flush();
}
pa += PAGE_SIZE_1G;
}
else if pa % PAGE_SIZE_2M == 0 && (pa + hhdm_offset) % PAGE_SIZE_2M == 0 && remaining >= PAGE_SIZE_2M
{
let vaddr = VirtAddr::from((pa + hhdm_offset) as usize);
mapper.map(vaddr, paddr, PageSize::Size2M, flags).expect("Failed to map 2M HHDM page").flush();

if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa as usize);
mapper.map(identity_vaddr, paddr, PageSize::Size2M, flags).expect("Failed to identity map 2M low page").flush();
}
pa += PAGE_SIZE_2M;
}
else
{
let vaddr = VirtAddr::from((pa + hhdm_offset) as usize);
mapper.map(vaddr, paddr, PageSize::Size4K, flags).expect("Failed to map 4K HHDM page").flush();
if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa as usize);
mapper.map(identity_vaddr, paddr, PageSize::Size4K, flags).expect("Failed to identity map 4K low page").flush();
}
pa += crate::memory::PAGE_SIZE as u64;
}
}
}
log::info!("HHDM and low-memory identity mapping complete.");

// Second, map the kernel itself at its higher-half virtual address.
let kernel_paddr = PhysAddr::from(kernel_addr.physical_base() as usize);
let kernel_vaddr = VirtAddr::from(kernel_addr.virtual_base() as usize);
// Map 16MB for the kernel.
let kernel_size = (kernel_file.size() as usize + crate::memory::PAGE_SIZE - 1) & !(crate::memory::PAGE_SIZE - 1);
let kflags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;

for offset in (0..kernel_size).step_by(crate::memory::PAGE_SIZE)
{
let paddr = kernel_paddr + offset;
let vaddr = kernel_vaddr + offset;
mapper.map(vaddr, paddr, PageSize::Size4K, kflags).expect("Failed to map kernel page").flush();
}
log::info!("Kernel sections mapped.");

// The new page table is ready. Load it into CR3.
let root_paddr = mapper.root_paddr();
let frame = x86_64::structures::paging::PhysFrame::from_start_address(x86_64::PhysAddr::new(root_paddr.as_usize() as u64),).unwrap();

// This is the point of no return. After this instruction, the CPU
// uses our new page table for all memory access.
unsafe { Cr3::write(frame, Cr3Flags::empty()) };
log::info!("x86_64 paging initialized and activated.");
}
