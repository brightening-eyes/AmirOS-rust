use super::gdt;
use amir_hal::IrqContext;
use amir_mm::{FRAME_ALLOCATOR, HEAP_END, HEAP_START, PAGE_MAPPER};
use core::sync::atomic::{AtomicBool, Ordering};
use free_list::PageLayout;
use lazy_static::lazy_static;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use spin::Mutex;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Callback invoked for an exception with a registered handler.
pub type ExceptionHandler = fn(&mut IrqContext);

/// Human-readable names for the 32 CPU exception vectors.
const EXCEPTION_NAMES: [&str; 32] = [
    "#DE divide error",
    "#DB debug",
    "NMI",
    "#BP breakpoint",
    "#OF overflow",
    "#BR bound range exceeded",
    "#UD invalid opcode",
    "#NM device not available",
    "#DF double fault",
    "(reserved 9)",
    "#TS invalid TSS",
    "#NP segment not present",
    "#SS stack segment fault",
    "#GP general protection fault",
    "#PF page fault",
    "(reserved 15)",
    "#MF x87 floating point",
    "#AC alignment check",
    "#MC machine check",
    "#XM SIMD floating point",
    "#VE virtualization",
    "#CP control protection",
    "(reserved 22)",
    "(reserved 23)",
    "(reserved 24)",
    "(reserved 25)",
    "(reserved 26)",
    "(reserved 27)",
    "#HV hypervisor injection",
    "#VC VMM communication",
    "#SX security exception",
    "(reserved 31)",
];

/// Per-vector registry of exception handlers.
///
/// Handlers registered here run after the built-in behaviors (see
/// [`handle_exception`]) and before the default dump-and-panic path.
static HANDLERS: [Mutex<Option<ExceptionHandler>>; 32] = [const { Mutex::new(None) }; 32];

/// Registers `handler` for CPU exception `vector`.
///
/// The handler runs only when the built-in behavior for that vector does not
/// finish the event:
/// - `#BP` (3) always logs and returns; registrations are never consulted.
/// - `#PF` (14) first services heap demand paging (the kernel heap depends on
///   it), then consults the registered hook for all other faults.
/// - `#DF` (8) is always fatal and cannot be overridden.
///
/// # Errors
/// Returns the handler back unchanged when the vector is reserved (`#DF`),
/// out of range, or already taken.
pub fn set_exception_handler(
    vector: u8,
    handler: ExceptionHandler,
) -> Result<(), ExceptionHandler> {
    let idx = usize::from(vector);
    if vector == 8 || idx >= 32 {
        return Err(handler);
    }
    let mut slot = HANDLERS[idx].lock();
    if slot.is_some() {
        return Err(handler);
    }
    *slot = Some(handler);
    Ok(())
}

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
        idt.divide_error.set_handler_fn(divide_error);
        idt.debug.set_handler_fn(debug);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt);
        idt.breakpoint.set_handler_fn(breakpoint);
        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available
            .set_handler_fn(device_not_available);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(segment_not_present);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault);
        idt.page_fault.set_handler_fn(page_fault);
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.machine_check.set_handler_fn(machine_check);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.cp_protection_exception
            .set_handler_fn(cp_protection_exception);
        idt.vmm_communication_exception
            .set_handler_fn(vmm_communication_exception);
        idt.security_exception.set_handler_fn(security_exception);

        // External interrupt vectors: LAPIC timer tick + every remapped PIC
        // line (spurious-aware) + one trampoline per routable GSI vector.
        // Inert until init_platform() brings the controllers up.
        idt[super::timer::TIMER_VECTOR]
            .set_handler_fn(super::timer::tick_handler);
        super::irq::install(&mut idt);
        super::ioapic::install(&mut idt);

        idt
    };
}

// Thin per-vector trampolines. Error-code exceptions funnel the raw code
// through so a single dispatcher owns logging, hooks, and panics.

extern "x86-interrupt" fn divide_error(frame: InterruptStackFrame) {
    handle_exception(0, &frame, None);
}
extern "x86-interrupt" fn debug(frame: InterruptStackFrame) {
    handle_exception(1, &frame, None);
}
extern "x86-interrupt" fn non_maskable_interrupt(frame: InterruptStackFrame) {
    handle_exception(2, &frame, None);
}
extern "x86-interrupt" fn breakpoint(frame: InterruptStackFrame) {
    handle_exception(3, &frame, None);
}
extern "x86-interrupt" fn overflow(frame: InterruptStackFrame) {
    handle_exception(4, &frame, None);
}
extern "x86-interrupt" fn bound_range_exceeded(frame: InterruptStackFrame) {
    handle_exception(5, &frame, None);
}
extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrame) {
    handle_exception(6, &frame, None);
}
extern "x86-interrupt" fn device_not_available(frame: InterruptStackFrame) {
    handle_exception(7, &frame, None);
}
extern "x86-interrupt" fn invalid_tss(frame: InterruptStackFrame, code: u64) {
    handle_exception(10, &frame, Some(code));
}
extern "x86-interrupt" fn segment_not_present(frame: InterruptStackFrame, code: u64) {
    handle_exception(11, &frame, Some(code));
}
extern "x86-interrupt" fn stack_segment_fault(frame: InterruptStackFrame, code: u64) {
    handle_exception(12, &frame, Some(code));
}
extern "x86-interrupt" fn general_protection_fault(frame: InterruptStackFrame, code: u64) {
    handle_exception(13, &frame, Some(code));
}
extern "x86-interrupt" fn page_fault(frame: InterruptStackFrame, code: PageFaultErrorCode) {
    handle_exception(14, &frame, Some(code.bits()));
}
extern "x86-interrupt" fn x87_floating_point(frame: InterruptStackFrame) {
    handle_exception(16, &frame, None);
}
extern "x86-interrupt" fn alignment_check(frame: InterruptStackFrame, code: u64) {
    handle_exception(17, &frame, Some(code));
}
extern "x86-interrupt" fn machine_check(frame: InterruptStackFrame) -> ! {
    // #MC aborts the machine; there is no meaningful continuation.
    fatal_dump(18, &frame, None);
}
extern "x86-interrupt" fn simd_floating_point(frame: InterruptStackFrame) {
    handle_exception(19, &frame, None);
}
extern "x86-interrupt" fn virtualization(frame: InterruptStackFrame) {
    handle_exception(20, &frame, None);
}
extern "x86-interrupt" fn cp_protection_exception(frame: InterruptStackFrame, code: u64) {
    handle_exception(21, &frame, Some(code));
}
extern "x86-interrupt" fn vmm_communication_exception(frame: InterruptStackFrame, code: u64) {
    handle_exception(29, &frame, Some(code));
}
extern "x86-interrupt" fn security_exception(frame: InterruptStackFrame, code: u64) {
    handle_exception(30, &frame, Some(code));
}

extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, _code: u64) -> ! {
    fatal_dump(8, &frame, None);
}

/// Common dispatch for every exception except `#DF`/`#MC`.
fn handle_exception(vector: u8, frame: &InterruptStackFrame, code: Option<u64>) {
    match vector {
        // Breakpoints are diagnostic: log and continue, as before.
        3 => {
            log::info!("EXCEPTION: BREAKPOINT");
            return;
        }
        // Demand-page the kernel heap first — allocations fault by design.
        14 => {
            let fault_addr = Cr2::read().expect("Cr2 is valid").as_u64() as usize;
            if (HEAP_START..=HEAP_END).contains(&fault_addr) && demand_page_heap(fault_addr) {
                return;
            }
        }
        _ => {}
    }

    let mut ctx = IrqContext {
        fault_addr: read_fault_addr(vector),
    };
    if let Some(handler) = *HANDLERS[usize::from(vector)].lock() {
        handler(&mut ctx);
        return;
    }

    fatal_dump(vector, frame, code);
}

/// Services a heap-range page fault by allocating a physical frame and
/// mapping it on demand, so that SlabHeap::new() and subsequent allocations
/// can proceed without pre-allocating physical memory for the whole heap.
/// Panics on out-of-memory; returns normally once the faulting page is mapped.
fn demand_page_heap(fault_addr: usize) -> bool {
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

    true
}

fn read_fault_addr(vector: u8) -> Option<VirtAddr> {
    if vector == 14 {
        Cr2::read()
            .ok()
            .map(|addr| VirtAddr::from(addr.as_u64() as usize))
    } else {
        None
    }
}

/// Logs the full exception context and panics — no registered recovery.
#[cold]
fn fatal_dump(vector: u8, frame: &InterruptStackFrame, code: Option<u64>) -> ! {
    let name = EXCEPTION_NAMES[usize::from(vector)];
    let cr2 = Cr2::read().map(|a| a.as_u64()).unwrap_or(0);
    panic!("unhandled exception {vector} ({name}): error_code={code:?}, cr2={cr2:#x}\n{frame:#?}",);
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
