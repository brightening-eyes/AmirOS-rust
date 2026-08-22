//! 8259A PIC handling: remap away from exception vectors, then mask every
//! line. The IO-APIC (discovered via MADT) owns external interrupts once the
//! platform initializes; the PIC stays fully masked but must be remapped
//! first so stray spurious lines cannot collide with CPU exceptions.

use x86_64::instructions::port::Port;

const MASTER_CMD: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_CMD: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;

/// Base vectors the PIC is remapped to (right above the CPU exceptions).
pub const PIC_MASTER_BASE: u8 = 0x20;
pub const PIC_SLAVE_BASE: u8 = 0x28;

/// Remaps both PICs to `PIC_{MASTER,SLAVE}_BASE` and masks all 15 usable
/// lines (including the cascade), leaving external interrupt delivery to the
/// IO-APIC.
pub fn init() {
    unsafe {
        // ICW1: start initialization, cascade mode, ICW4 needed.
        Port::<u8>::new(MASTER_CMD).write(0x11);
        Port::<u8>::new(SLAVE_CMD).write(0x11);
        // ICW2: vector offsets.
        Port::<u8>::new(MASTER_DATA).write(PIC_MASTER_BASE);
        Port::<u8>::new(SLAVE_DATA).write(PIC_SLAVE_BASE);
        // ICW3: slave hangs off master line 2; slave identifies as line 2.
        Port::<u8>::new(MASTER_DATA).write(0x04);
        Port::<u8>::new(SLAVE_DATA).write(0x02);
        // ICW4: 8086 mode.
        Port::<u8>::new(MASTER_DATA).write(0x01);
        Port::<u8>::new(SLAVE_DATA).write(0x01);
        // OCW1: mask everything.
        Port::<u8>::new(MASTER_DATA).write(0xFF);
        Port::<u8>::new(SLAVE_DATA).write(0xFF);
    }
    log::info!(
        "PIC remapped to {:#x}-{:#x} and fully masked",
        PIC_MASTER_BASE,
        PIC_SLAVE_BASE + 7
    );
}

/// Reads the In-Service Register of both PICs.
fn read_isr() -> (u8, u8) {
    unsafe {
        Port::<u8>::new(MASTER_CMD).write(0x0B); // OCW3: next read returns ISR
        Port::<u8>::new(SLAVE_CMD).write(0x0B);
        (
            Port::<u8>::new(MASTER_CMD).read(),
            Port::<u8>::new(SLAVE_CMD).read(),
        )
    }
}

/// Returns `true` if IRQ `line` (7 or 15) is a spurious interrupt that needs
/// no EOI. Real spurious lines are reported without being set in the ISR.
pub fn take_spurious(vector: u8) -> bool {
    let (master_isr, slave_isr) = read_isr();
    match vector {
        v if v == PIC_MASTER_BASE + 7 => master_isr & (1 << 7) == 0,
        v if v == PIC_SLAVE_BASE + 7 => slave_isr & (1 << 7) == 0,
        _ => false,
    }
}

/// Non-specific EOI to the master PIC.
pub fn eoi_master() {
    unsafe { Port::<u8>::new(MASTER_CMD).write(0x20) };
}

/// EOI for slave-serviced lines: the slave first, then the master.
pub fn eoi_slave() {
    unsafe {
        Port::<u8>::new(SLAVE_CMD).write(0x20);
        Port::<u8>::new(MASTER_CMD).write(0x20);
    }
}
