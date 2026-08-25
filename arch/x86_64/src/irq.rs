use amir_hal::{IrqContext, IrqNumber};
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

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

// --- Controller mode ---------------------------------------------------------

/// `true` once an IO-APIC topology is live and the PICs stay fully masked.
/// Until then external lines are delivered by the remapped PICs.
static APIC_MODE: AtomicBool = AtomicBool::new(false);

/// Switches the controller mode. Called once from [`super::init_platform`].
pub fn set_apic_mode(on: bool) {
    APIC_MODE.store(on, Ordering::Release);
}

/// Returns `true` when external lines route through the IO-APIC.
#[must_use]
pub fn apic_mode() -> bool {
    APIC_MODE.load(Ordering::Acquire)
}

/// Maps an ISA hardware line to its GSI, honoring MADT overrides.
fn isa_line_to_gsi(line: u8) -> Option<u32> {
    super::acpi::platform().map(|p| p.isa_override(line).map_or(u32::from(line), |o| o.gsi))
}

// --- hal::IrqController implementation --------------------------------------

/// x86_64 interrupt controller for the HAL registry.
///
/// The HAL namespace is ISA hardware lines (0–15): the common denominator of
/// both supported controllers. In APIC mode lines are translated through MADT
/// overrides onto GSI redirection entries; in legacy mode they map directly
/// onto the remapped PIC vectors. PCI-native GSIs bypass this API via
/// [`super::ioapic::register_gsi`] once MSI/IO-APIC consumers exist.
///
/// Handlers run in interrupt context with the dispatcher owning EOI —
/// registered handlers never signal it themselves.
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
        irq: IrqNumber,
        handler: fn(&mut IrqContext),
    ) -> Result<(), fn(&mut IrqContext)> {
        if irq.0 >= 16 {
            return Err(handler);
        }
        let line = irq.0 as u8;
        if apic_mode() {
            match isa_line_to_gsi(line) {
                Some(gsi) => super::ioapic::register_gsi(gsi, handler),
                None => Err(handler),
            }
        } else {
            register_pic_line(line, handler)
        }
    }

    fn eoi(&self, _irq: IrqNumber) {
        // The per-line dispatchers EOI automatically; this path exists for
        // consumers that take over a line manually in APIC mode.
        if apic_mode() {
            super::lapic::eoi();
        }
    }
}

// --- Legacy PIC dispatch (vectors PIC_{MASTER,SLAVE}_BASE..) -----------------

type IrqHandler = fn(&mut IrqContext);

static PIC_HANDLER_TABLE: [spin::Mutex<Option<IrqHandler>>; 16] =
    [const { spin::Mutex::new(None) }; 16];

static MASTER_SPURIOUS_SEEN: AtomicBool = AtomicBool::new(false);
static SLAVE_SPURIOUS_SEEN: AtomicBool = AtomicBool::new(false);

/// Logs a spurious PIC line exactly once per line.
pub fn log_spurious_once(vector: u8, seen: &AtomicBool) {
    if !seen.swap(true, Ordering::AcqRel) {
        log::warn!("spurious PIC interrupt on vector {vector:#x} (masked, ignored)");
    }
}

/// Dispatches one remapped PIC vector.
fn dispatch_pic(line: u8) {
    // Lines 7/15 can be spurious: reported without being set in the ISR.
    let spurious_vector = match line {
        7 => Some(super::pic::PIC_MASTER_BASE + 7),
        15 => Some(super::pic::PIC_SLAVE_BASE + 7),
        _ => None,
    };
    if let Some(vector) = spurious_vector {
        let seen = if line == 7 {
            &MASTER_SPURIOUS_SEEN
        } else {
            &SLAVE_SPURIOUS_SEEN
        };
        if super::pic::take_spurious(vector) {
            log_spurious_once(vector, seen);
            return; // No EOI for spurious lines.
        }
    }

    let mut ctx = IrqContext { fault_addr: None };
    if let Some(handler) = *PIC_HANDLER_TABLE[usize::from(line)].lock() {
        handler(&mut ctx);
    } else {
        log::warn!("pic: unhandled IRQ {line}");
    }

    if line >= 8 {
        super::pic::eoi_slave();
    } else {
        super::pic::eoi_master();
    }
}

/// Registers `handler` on legacy PIC `line`, unmasking it. Runs with
/// interrupts disabled.
///
/// # Errors
/// Returns the handler back unchanged when the line is taken or reserved.
fn register_pic_line(line: u8, handler: IrqHandler) -> Result<(), IrqHandler> {
    interrupts::without_interrupts(|| {
        let mut slot = PIC_HANDLER_TABLE[usize::from(line)].lock();
        if slot.is_some() {
            return Err(handler);
        }
        *slot = Some(handler);
        drop(slot);
        super::pic::unmask(line);
        Ok(())
    })
}

macro_rules! pic_trampolines {
    ($($name:ident = $line:literal),* $(,)?) => {
        $(
            extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
                dispatch_pic($line);
            }
        )*
        /// Wires all 15 usable PIC vectors into the IDT. Inert while every
        /// line stays masked (APIC mode).
        pub(crate) fn install(idt: &mut x86_64::structures::idt::InterruptDescriptorTable) {
            $(
                idt[pic_vector($line)].set_handler_fn($name);
            )*
        }
    };
}

/// The remapped vector delivering ISA `line`.
const fn pic_vector(line: u8) -> u8 {
    if line < 8 {
        super::pic::PIC_MASTER_BASE + line
    } else {
        super::pic::PIC_SLAVE_BASE + (line - 8)
    }
}

pic_trampolines!(
    pic_line00 = 0x00,
    pic_line01 = 0x01,
    pic_line02 = 0x02,
    pic_line03 = 0x03,
    pic_line04 = 0x04,
    pic_line05 = 0x05,
    pic_line06 = 0x06,
    pic_line07 = 0x07,
    pic_line08 = 0x08,
    pic_line09 = 0x09,
    pic_line0a = 0x0A,
    pic_line0b = 0x0B,
    pic_line0c = 0x0C,
    pic_line0d = 0x0D,
    pic_line0e = 0x0E,
    pic_line0f = 0x0F,
);
