use amir_hal::{IrqContext, IrqNumber};
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

// --- Port IO helpers (shared by pic/pit/acpi) -------------------------------

pub fn port_read_u8(port: u16) -> u8 {
    unsafe { Port::new(port).read() }
}
pub fn port_read_u16(port: u16) -> u16 {
    unsafe { Port::new(port).read() }
}
pub fn port_read_u32(port: u16) -> u32 {
    unsafe { Port::new(port).read() }
}
pub fn port_write_u8(port: u16, value: u8) {
    unsafe { Port::new(port).write(value) }
}
pub fn port_write_u16(port: u16, value: u16) {
    unsafe { Port::new(port).write(value) }
}
pub fn port_write_u32(port: u16, value: u32) {
    unsafe { Port::new(port).write(value) }
}

// --- hal::IrqController implementation --------------------------------------

/// x86_64 interrupt controller state for the HAL registry.
///
/// External IRQ routing (IO-APIC redirection entries) lands with the first
/// real device consumers; until then registration is rejected explicitly
/// rather than silently dropping lines.
pub struct X86IrqController;

impl amir_hal::IrqController for X86IrqController {
    fn enable(&self) {
        interrupts::enable();
    }

    fn disable(&self) {
        interrupts::disable();
    }

    fn register(
        &self,
        _irq: IrqNumber,
        handler: fn(&mut IrqContext),
    ) -> Result<(), fn(&mut IrqContext)> {
        Err(handler)
    }

    fn eoi(&self, _irq: IrqNumber) {
        super::lapic::eoi();
    }
}

/// Logs a spurious PIC line exactly once per line.
pub fn log_spurious_once(vector: u8, seen: &core::sync::atomic::AtomicBool) {
    use core::sync::atomic::Ordering;
    if !seen.swap(true, Ordering::AcqRel) {
        log::warn!("spurious PIC interrupt on vector {vector:#x} (masked, ignored)");
    }
}

// --- Spurious PIC vector handlers (wired into the IDT) ----------------------

static MASTER_SPURIOUS_SEEN: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static SLAVE_SPURIOUS_SEEN: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

pub(crate) extern "x86-interrupt" fn pic_master_spurious(
    _frame: x86_64::structures::idt::InterruptStackFrame,
) {
    let vector = super::pic::PIC_MASTER_BASE + 7;
    if super::pic::take_spurious(vector) {
        log_spurious_once(vector, &MASTER_SPURIOUS_SEEN);
    } else {
        super::pic::eoi_master();
    }
}

pub(crate) extern "x86-interrupt" fn pic_slave_spurious(
    _frame: x86_64::structures::idt::InterruptStackFrame,
) {
    let vector = super::pic::PIC_SLAVE_BASE + 7;
    if super::pic::take_spurious(vector) {
        log_spurious_once(vector, &SLAVE_SPURIOUS_SEEN);
    } else {
        super::pic::eoi_slave();
    }
}
