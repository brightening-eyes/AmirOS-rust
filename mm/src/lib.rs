//! AmirOS memory management: frame allocation, paging, and the slab heap.
#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use limine::memmap::{Entry, MEMMAP_BAD_MEMORY};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use spin::RwLock;

pub mod allocator;
pub mod heap;
pub mod paging;

pub use heap::{HEAP_END, HEAP_SIZE, HEAP_START};

pub type PageTable = paging::PageTable;
pub type PageTableEntry = paging::PageTableEntry;

/// Higher-half direct map offset, stored here by [`init`] before anything
/// can touch the frame allocator or page mapper.
static HHDM_OFFSET: AtomicUsize = AtomicUsize::new(0);

fn hhdm_offset() -> usize {
    HHDM_OFFSET.load(Ordering::Acquire)
}

lazy_static! {
    pub static ref FRAME_ALLOCATOR: RwLock<allocator::FrameAllocator> =
        RwLock::new(allocator::FrameAllocator::new(hhdm_offset()));
    pub static ref PAGE_MAPPER: RwLock<PageTable> = {
        let page_table = PageTable::try_new().expect("Failed to create page table");
        RwLock::new(page_table)
    };
}

pub const PAGE_SIZE_1G: usize = 1024 * 1024 * 1024;
pub const PAGE_SIZE_2M: usize = 2 * 1024 * 1024;
pub const PAGE_SIZE: usize = 4096;

/// initialization code for the memory manager and page mapping.
///
/// `hhdm_offset`, `kernel_physical_base`, `kernel_virtual_base`, and
/// `kernel_file_size` are provided by the kernel binary from its Limine boot
/// responses, keeping this crate free of bootloader globals.
///
/// # Panics
/// if initialization fails or we cant map the kernel.
#[allow(clippy::too_many_arguments)]
pub fn init(
    memmap: &[&Entry],
    hhdm_offset_raw: u64,
    kernel_physical_base: u64,
    kernel_virtual_base: u64,
    kernel_file_size: usize,
) {
    let hhdm_offset = usize::try_from(hhdm_offset_raw).expect("memory: invalid HHDM offset");
    HHDM_OFFSET.store(hhdm_offset, Ordering::Release);

    // initialize our frame allocator.
    FRAME_ALLOCATOR.write().init(memmap);

    let mut mapper = PAGE_MAPPER.write();
    let flags = MappingFlags::READ | MappingFlags::WRITE;

    // First, map all physical memory to the higher-half direct map (HHDM) region.
    // We also identity-map the first 4GiB. This is a robust technique to ensure
    // that the CPU can continue execution seamlessly after the CR3 switch, as it
    // makes physical addresses temporarily valid as virtual addresses.
    for entry in memmap {
        // We map all memory types except for bad memory. This includes the kernel,
        // modules, and bootloader-reclaimable memory.
        if matches!(entry.type_, MEMMAP_BAD_MEMORY) {
            continue;
        }
        let start_pa = usize::try_from(entry.base).expect("memory: invalid base in memmap entry");
        let length = usize::try_from(entry.length).expect("memory: invalid length in memmap entry");
        let end_pa = start_pa
            .checked_add(length)
            .expect("memory: integer overflow in memmap range calculation");
        let mut pa = start_pa;

        while pa < end_pa {
            let remaining = end_pa - pa;
            let paddr = PhysAddr::from(pa);

            // Prioritize the largest possible page size.
            if pa.is_multiple_of(PAGE_SIZE_1G)
                && (pa + hhdm_offset).is_multiple_of(PAGE_SIZE_1G)
                && remaining >= PAGE_SIZE_1G
            {
                let vaddr = VirtAddr::from(pa + hhdm_offset);
                mapper
                    .cursor()
                    .map(vaddr, paddr, PageSize::Size1G, flags)
                    .expect("Failed to map 1G HHDM page");
                if pa < 0x1_0000_0000 {
                    let identity_vaddr = VirtAddr::from(pa);
                    mapper
                        .cursor()
                        .map(identity_vaddr, paddr, PageSize::Size1G, flags)
                        .expect("Failed to identity map 1G low page");
                }
                pa += PAGE_SIZE_1G;
            } else if pa.is_multiple_of(PAGE_SIZE_2M)
                && (pa + hhdm_offset).is_multiple_of(PAGE_SIZE_2M)
                && remaining >= PAGE_SIZE_2M
            {
                let vaddr = VirtAddr::from(pa + hhdm_offset);
                mapper
                    .cursor()
                    .map(vaddr, paddr, PageSize::Size2M, flags)
                    .expect("Failed to map 2M HHDM page");
                if pa < 0x1_0000_0000 {
                    let identity_vaddr = VirtAddr::from(pa);
                    mapper
                        .cursor()
                        .map(identity_vaddr, paddr, PageSize::Size2M, flags)
                        .expect("Failed to identity map 2M low page");
                }
                pa += PAGE_SIZE_2M;
            } else {
                let vaddr = VirtAddr::from(pa + hhdm_offset);
                mapper
                    .cursor()
                    .map(vaddr, paddr, PageSize::Size4K, flags)
                    .expect("Failed to map 4K HHDM page");
                if pa < 0x1_0000_0000 {
                    let identity_vaddr = VirtAddr::from(pa);
                    mapper
                        .cursor()
                        .map(identity_vaddr, paddr, PageSize::Size4K, flags)
                        .expect("Failed to identity map 4K low page");
                }
                pa += PAGE_SIZE;
            }
        }
    }
    log::info!("HHDM and low-memory identity mapping complete.");

    // Second, map the kernel itself at its higher-half virtual address.
    let kernel_physical_address = PhysAddr::from(
        usize::try_from(kernel_physical_base)
            .expect("memory: invalid kernel physical base address"),
    );
    let kernel_virtual_address = VirtAddr::from(
        usize::try_from(kernel_virtual_base).expect("memory: invalid kernel virtual base address"),
    );
    let kernel_size = (kernel_file_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let kflags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;
    for offset in (0..kernel_size).step_by(PAGE_SIZE) {
        let paddr = kernel_physical_address + offset;
        let vaddr = kernel_virtual_address + offset;
        mapper
            .cursor()
            .map(vaddr, paddr, PageSize::Size4K, kflags)
            .expect("Failed to map kernel page");
    }
    log::info!("Kernel sections mapped.");
}
