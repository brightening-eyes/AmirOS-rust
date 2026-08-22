//! Unified, multi-architecture paging using a single handler.
use crate::FRAME_ALLOCATOR;
use core::alloc::Layout;
use free_list::PageLayout;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::PagingHandler;

#[derive(Clone)]
pub struct AmirOSPagingHandler;

impl PagingHandler for AmirOSPagingHandler {
    fn alloc_frames(num_pages: usize, align: usize) -> Option<PhysAddr> {
        let size = num_pages
            .checked_mul(0x1000)
            .expect("paging: integer overflow in alloc_frames size");
        let layout: PageLayout = PageLayout::from_size_align(size, align)
            .expect("paging: invalid page layout for alloc_frames");
        let mut allocator = FRAME_ALLOCATOR.write();
        if let Ok(page_range) = allocator.allocate(layout) {
            let paddr = page_range.start();
            Some(PhysAddr::from(paddr))
        } else {
            None
        }
    }

    fn dealloc_frames(paddr: PhysAddr, num_pages: usize) {
        let size = num_pages
            .checked_mul(0x1000)
            .expect("paging: integer overflow in dealloc_frames size");
        let layout = Layout::from_size_align(size, 0x1000)
            .expect("paging: invalid layout for dealloc_frames");
        let mut allocator = FRAME_ALLOCATOR.write();
        let paddr_start = paddr.as_usize();
        let paddr_end = paddr_start
            .checked_add(layout.size())
            .expect("paging: integer overflow in dealloc_frames range");
        if let Ok(page_range) = (paddr_start..paddr_end).try_into() {
            allocator.deallocate(page_range);
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        let pa = paddr.as_usize();
        pa.checked_add(FRAME_ALLOCATOR.read().hhdm_offset)
            .map(VirtAddr::from_usize)
            .expect("failed to allocate address")
    }
}

/// The concrete page table used by the running architecture, instantiated
/// over the shared [`AmirOSPagingHandler`]. This is the single source of
/// truth for the type; per-architecture crates consume it via `amir_mm`.
#[cfg(target_arch = "x86_64")]
pub type PageTable = page_table_multiarch::x86_64::X64PageTable<AmirOSPagingHandler>;
#[cfg(target_arch = "x86_64")]
pub type PageTableEntry = page_table_entry::x86_64::X64PTE;

#[cfg(target_arch = "riscv64")]
pub type PageTable = page_table_multiarch::riscv::Sv48PageTable<AmirOSPagingHandler>;
#[cfg(target_arch = "riscv64")]
pub type PageTableEntry = page_table_entry::riscv::Rv64PTE;

#[cfg(target_arch = "aarch64")]
pub type PageTable = page_table_multiarch::aarch64::A64PageTable<AmirOSPagingHandler>;
#[cfg(target_arch = "aarch64")]
pub type PageTableEntry = page_table_entry::aarch64::A64PTE;

#[cfg(target_arch = "loongarch64")]
pub type PageTable = page_table_multiarch::loongarch64::LA64PageTable<AmirOSPagingHandler>;
#[cfg(target_arch = "loongarch64")]
pub type PageTableEntry = page_table_entry::loongarch64::LA64PTE;
