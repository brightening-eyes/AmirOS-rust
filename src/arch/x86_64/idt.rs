use super::gdt;
use crate::allocator::{HEAP_SIZE, HEAP_START};
use crate::memory::{FRAME_ALLOCATOR, PAGE_MAPPER};
use free_list::PageLayout;
use lazy_static::lazy_static;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    log::info!("EXCEPTION: BREAKPOINT");
}

extern "x86-interrupt" fn page_fault_handler(
    _frame: InterruptStackFrame,
    _error_code: PageFaultErrorCode,
) {
    let fault_addr = Cr2::read().expect("Cr2 is valid").as_u64() as usize;

    // Demand-page the heap region: allocate a physical frame and map it on
    // the first access, so that SlabHeap::new() and subsequent allocations
    // can proceed without pre-allocating physical memory for the whole heap.
    if fault_addr >= HEAP_START && fault_addr < HEAP_START + HEAP_SIZE {
        let page_addr = fault_addr & !0xFFF;
        let vaddr = VirtAddr::from(page_addr);

        let Ok(layout) = PageLayout::from_size_align(4096, 4096) else {
            panic!("heap: invalid page layout for demand paging");
        };

        // Allocate a physical frame. Drop the lock before mapping so that
        // cursor.map() can acquire FRAME_ALLOCATOR for page-table pages.
        let paddr = {
            let mut frame_alloc = FRAME_ALLOCATOR
                .try_write()
                .expect("heap: FRAME_ALLOCATOR contention in page fault handler");
            let range = frame_alloc
                .allocate(layout)
                .expect("heap: out of physical memory for demand paging");
            PhysAddr::from(range.start())
        };

        {
            let mut mapper = PAGE_MAPPER
                .try_write()
                .expect("heap: PAGE_MAPPER contention in page fault handler");
            mapper
                .cursor()
                .map(
                    vaddr,
                    paddr,
                    PageSize::Size4K,
                    MappingFlags::READ | MappingFlags::WRITE,
                )
                .expect("heap: failed to map page on demand");
        }

        return;
    }

    panic!(
        "Page fault at {:#x}, error code: {:?}\n{:#?}",
        fault_addr, _error_code, _frame
    );
}

extern "x86-interrupt" fn double_fault_handler(_stack: InterruptStackFrame, _error_code: u64) -> ! {
    panic!("double fault!.");
}

pub fn init() {
    IDT.load();
}
