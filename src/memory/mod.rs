// memory management
use core::alloc::Layout;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize, GenericPTE};
use limine::memory_map::{Entry, EntryType};
use lazy_static::lazy_static;
use spin::Mutex;
use crate::memory::vmm::VirtualMemoryArea;
use crate::arch;
pub mod allocator;
pub mod paging;
pub mod vmm;

pub type PageTable = crate::arch::PageTable;
pub type PageTableEntry = arch::PageTableEntry;

lazy_static!
{
pub static ref FRAME_ALLOCATOR: Mutex<allocator::FrameAllocator> = {
let hhdm_offset = crate::HHDM_REQUEST.get_response().unwrap().offset() as usize;
Mutex::new(allocator::FrameAllocator::new(hhdm_offset))
};
pub static ref PAGE_MAPPER: Mutex<PageTable> = {
let page_table = PageTable::try_new().expect("Failed to create x86_64 page table");
Mutex::new(page_table)
    };
pub static ref VIRTUAL_ADDRESS_SPACE: Mutex<vmm::VirtualAddressSpace> = {
Mutex::new(vmm::VirtualAddressSpace::new())
};
}

pub static PAGE_SIZE_1G: usize = 1024 * 1024 * 1024;
pub static PAGE_SIZE_2M: usize = 2 * 1024 * 1024;
pub static PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy)]
struct MappingInfo
{
vaddr: VirtAddr,
size: usize,
area: VirtualMemoryArea,
}

pub fn init(memmap: &[&Entry])
{
// initialize our frame allocator.
FRAME_ALLOCATOR.lock().init(memmap);
// Get the necessary information from the bootloader.
let hhdm_offset = FRAME_ALLOCATOR.lock().hhdm_offset;
let kernel_addr = crate::EXECUTABLE_ADDRESS_REQUEST.get_response().unwrap();
let kernel_file = crate::EXECUTABLE_FILE_REQUEST.get_response().unwrap().file();
let mut mapper = PAGE_MAPPER.lock();
let flags = MappingFlags::READ | MappingFlags::WRITE;

// First, map all physical memory to the higher-half direct map (HHDM) region.
// We also identity-map the first 4GiB. This is a robust technique to ensure
// that the CPU can continue execution seamlessly after the CR3 switch, as it
// makes physical addresses temporarily valid as virtual addresses.
for entry in memmap.iter()
{
// We map all memory types except for bad memory. This includes the kernel,
// modules, and bootloader-reclaimable memory.
if matches!(entry.entry_type, EntryType::BAD_MEMORY)
{
continue;
}
let start_pa = entry.base as usize;
let end_pa = start_pa + entry.length as usize;
let mut pa = start_pa;

while pa < end_pa
{
let remaining = end_pa - pa;
let paddr = PhysAddr::from(pa);

// Prioritize the largest possible page size.
if pa.is_multiple_of(PAGE_SIZE_1G) && (pa + hhdm_offset).is_multiple_of(PAGE_SIZE_1G) && remaining >= PAGE_SIZE_1G
{
let vaddr = VirtAddr::from(pa + hhdm_offset);
mapper.map(vaddr, paddr, PageSize::Size1G, flags).expect("Failed to map 1G HHDM page").flush();
if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa);
mapper.map(identity_vaddr, paddr, PageSize::Size1G, flags).expect("Failed to identity map 1G low page").flush();
}
pa += PAGE_SIZE_1G;
}
else if pa.is_multiple_of(PAGE_SIZE_2M) && (pa + hhdm_offset).is_multiple_of(PAGE_SIZE_2M) && remaining >= PAGE_SIZE_2M
{
let vaddr = VirtAddr::from(pa + hhdm_offset);
mapper.map(vaddr, paddr, PageSize::Size2M, flags).expect("Failed to map 2M HHDM page").flush();
if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa);
mapper.map(identity_vaddr, paddr, PageSize::Size2M, flags).expect("Failed to identity map 2M low page").flush();
}
pa += PAGE_SIZE_2M;
}
else
{
let vaddr = VirtAddr::from(pa + hhdm_offset);
mapper.map(vaddr, paddr, PageSize::Size4K, flags).expect("Failed to map 4K HHDM page").flush();
if pa < 0x1_0000_0000
{
let identity_vaddr = VirtAddr::from(pa);
mapper.map(identity_vaddr, paddr, PageSize::Size4K, flags).expect("Failed to identity map 4K low page").flush();
}
pa += PAGE_SIZE;
}
}
}
log::info!("HHDM and low-memory identity mapping complete.");

// Second, map the kernel itself at its higher-half virtual address.
let kernel_paddr = PhysAddr::from(kernel_addr.physical_base() as usize);
let kernel_vaddr = VirtAddr::from(kernel_addr.virtual_base() as usize);
let kernel_size = (kernel_file.size() as usize + crate::memory::PAGE_SIZE - 1) & !(crate::memory::PAGE_SIZE - 1);
let kflags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;
for offset in (0..kernel_size).step_by(crate::memory::PAGE_SIZE)
{
let paddr = kernel_paddr + offset;
let vaddr = kernel_vaddr + offset;
mapper.map(vaddr, paddr, PageSize::Size4K, kflags).expect("Failed to map kernel page").flush();
}
log::info!("Kernel sections mapped.");
}

pub fn init_vmm()
{
let mut mapper = PAGE_MAPPER.lock();
let closure = |level: usize, index: usize, address: VirtAddr, pte: &PageTableEntry|
{
let flags = pte.flags();
let area = VirtualMemoryArea { flags };
let size = match level
{
1 => PAGE_SIZE_1G,
2 => PAGE_SIZE_2M,
3 => PAGE_SIZE,
_ => 0,
};
if size > 0
{
VIRTUAL_ADDRESS_SPACE.lock().allocate(address, size, area);
}
};
mapper.walk(usize::MAX, Some(&closure), None).expect("could not walk the page mapper");
}

pub fn kernel_alloc(layout: Layout) -> Option<VirtAddr>
{
let mut vmas = VIRTUAL_ADDRESS_SPACE.lock();
let size = layout.size();
let flags = MappingFlags::READ | MappingFlags::WRITE;
let paddr = PhysAddr::from(FRAME_ALLOCATOR.lock().allocate(size).expect("Failed to allocate a frame for the heap.").start());
let vaddr = vmas.find_free_area(size).expect("failed to find a virtual address");
vmas.allocate(vaddr, size, VirtualMemoryArea { flags: flags });
PAGE_MAPPER.lock().map(vaddr, paddr, PageSize::Size4K, flags).expect("failed to map the page.").flush();
Some(vaddr)
}

pub fn kernel_dealloc(address: VirtAddr, layout: Layout)
{
let size = layout.size();
VIRTUAL_ADDRESS_SPACE.lock().dealloc(address, size);
let _ = PAGE_MAPPER.lock().unmap(address);
}
