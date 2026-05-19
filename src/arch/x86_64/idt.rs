use super::gdt;
use crate::allocator::{HEAP_SIZE, HEAP_START};
use crate::memory::{FRAME_ALLOCATOR, PAGE_MAPPER};
use core::sync::atomic::{AtomicBool, Ordering};
use free_list::PageLayout;
use lazy_static::lazy_static;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Pre-allocated emergency frame for the page fault handler.
/// Used when FRAME_ALLOCATOR is contended (e.g., the faulting code
/// holds the allocator lock). This avoids deadlock.
static EMERGENCY_FRAME: EmergencyFrame = EmergencyFrame::new();

struct EmergencyFrame {
    allocated: AtomicBool,
    paddr: core::cell::UnsafeCell<Option<PhysAddr>>,
}

// Safety: synchronization is provided by the AtomicBool gate on all
// access to the UnsafeCell contents. Only one thread can observe
// `allocated == true` and proceed to read the inner value.
unsafe impl Sync for EmergencyFrame {}

impl EmergencyFrame {
    const fn new() -> Self {
        Self {
            allocated: AtomicBool::new(false),
            paddr: core::cell::UnsafeCell::new(None),
        }
    }

    fn init(&self, paddr: PhysAddr) {
        // Safety: called once during init, no concurrent access.
        unsafe { *self.paddr.get() = Some(paddr) };
        self.allocated.store(true, Ordering::Release);
    }

    fn take(&self) -> Option<PhysAddr> {
        if self.allocated.swap(false, Ordering::AcqRel) {
            // Safety: we just verified through the atomic that the value is Some.
            Some(unsafe { (*self.paddr.get()).expect("x86_64: emergency frame address is None") })
        } else {
            None
        }
    }
}

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
    if (HEAP_START..HEAP_START + HEAP_SIZE).contains(&fault_addr) {
        let page_addr = fault_addr & !0xFFF;
        let vaddr = VirtAddr::from(page_addr);

        let Ok(layout) = PageLayout::from_size_align(4096, 4096) else {
            panic!("heap: invalid page layout for demand paging");
        };

        // Allocate a physical frame. Drop the lock before mapping so that
        // cursor.map() can acquire FRAME_ALLOCATOR for page-table pages.
        // Try the frame allocator first; fall back to emergency pool on
        // contention to avoid deadlock.
        let paddr = loop {
            if let Some(mut frame_alloc) = FRAME_ALLOCATOR.try_write() {
                let range = frame_alloc
                    .allocate(layout)
                    .expect("heap: out of physical memory for demand paging");
                break PhysAddr::from(range.start());
            }
            if let Some(emergency) = EMERGENCY_FRAME.take() {
                break emergency;
            }
            core::hint::spin_loop();
        };

        // Map the page with backoff on PAGE_MAPPER contention.
        loop {
            if let Some(mut mapper) = PAGE_MAPPER.try_write() {
                mapper
                    .cursor()
                    .map(
                        vaddr,
                        paddr,
                        PageSize::Size4K,
                        MappingFlags::READ | MappingFlags::WRITE,
                    )
                    .expect("heap: failed to map page on demand");
                break;
            }
            core::hint::spin_loop();
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
    // Pre-allocate an emergency physical frame for the page fault handler,
    // so it can service faults even when FRAME_ALLOCATOR is contended.
    let layout =
        PageLayout::from_size_align(4096, 4096).expect("x86_64: invalid emergency frame layout");
    let mut frame_alloc = FRAME_ALLOCATOR.write();
    if let Ok(range) = frame_alloc.allocate(layout) {
        EMERGENCY_FRAME.init(PhysAddr::from(range.start()));
    }
    drop(frame_alloc);

    IDT.load();
}
