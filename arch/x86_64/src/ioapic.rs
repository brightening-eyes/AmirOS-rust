//! IO-APIC driver: MMIO mapping, redirection-table programming, GSI routing,
//! and MSI/MSI-X message composition.
//!
//! External lines occupy vectors `EXTERNAL_VECTOR_BASE..` with one IDT
//! trampoline per slot. Handlers registered through `hal::irq` run in
//! interrupt context; the dispatcher performs the LAPIC EOI after the handler
//! returns. Deferred work belongs in `hal::bottom_half`.

use crate::{acpi, lapic, pit};
use amir_hal::IrqContext;
use amir_mm::PAGE_MAPPER;
use core::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

/// First external-interrupt vector. Above the LAPIC timer (`0x20`) and the
/// remapped PIC spurious range; `0x40..=0x5F` are reserved for routed GSIs.
pub const EXTERNAL_VECTOR_BASE: u8 = 0x40;
/// Number of external vectors backed by IDT trampolines.
pub const EXTERNAL_VECTOR_COUNT: usize = 32;

const MAX_IO_APICS: usize = 4;
/// Fixed virtual address of the first IO-APIC MMIO page — the next free
/// scratch slot above the LAPIC page (`0xFFFF_FF80_0000_0000`).
const IO_APIC_VADDR_BASE: usize = 0xFFFF_FF80_0000_1000;

// Redirection-entry bits (lower dword). Physical destination mode and fixed
// delivery both encode as zero.
const RINTPOL_LOW: u32 = 1 << 13;
const REDTRIG_LEVEL: u32 = 1 << 15;
const RINT_MASK: u32 = 1 << 16;

type HandlerFn = fn(&mut IrqContext);

struct IoApicRegs {
    vaddr: usize,
    gsi_base: u32,
    redir_count: u32,
}

static IO_APICS: Mutex<[Option<IoApicRegs>; MAX_IO_APICS]> =
    Mutex::new([const { None }; MAX_IO_APICS]);

static NEXT_VECTOR: AtomicU8 = AtomicU8::new(EXTERNAL_VECTOR_BASE);
/// GSI routed onto each external-vector slot; `u32::MAX` = free.
static ROUTED_GSI: [AtomicU32; EXTERNAL_VECTOR_COUNT] =
    [const { AtomicU32::new(u32::MAX) }; EXTERNAL_VECTOR_COUNT];

static HANDLERS: [Mutex<Option<HandlerFn>>; EXTERNAL_VECTOR_COUNT] =
    [const { Mutex::new(None) }; EXTERNAL_VECTOR_COUNT];

// --- Register access --------------------------------------------------------

#[inline]
fn io_regsel(vaddr: usize) -> *mut u32 {
    vaddr as *mut u32
}

#[inline]
fn io_window(vaddr: usize) -> *mut u32 {
    (vaddr + 0x10) as *mut u32
}

/// # Safety
/// `vaddr` must be a mapped IO-APIC MMIO page and `reg` a valid selector.
unsafe fn read_reg(vaddr: usize, reg: u8) -> u32 {
    unsafe {
        io_regsel(vaddr).write_volatile(u32::from(reg));
        io_window(vaddr).read_volatile()
    }
}

/// # Safety
/// `vaddr` must be a mapped IO-APIC MMIO page and `reg` a valid selector.
unsafe fn write_reg(vaddr: usize, reg: u8, value: u32) {
    unsafe {
        io_regsel(vaddr).write_volatile(u32::from(reg));
        io_window(vaddr).write_volatile(value)
    }
}

fn redirection_sel(pin: u32) -> u8 {
    u8::try_from(0x10 + 2 * pin).expect("ioapic: redirection selector overflow")
}

// --- Init -------------------------------------------------------------------

/// Maps every MADT-discovered IO-APIC and masks all of its redirection
/// entries so unclaimed GSIs stay silent until something routes them.
///
/// # Panics
/// Panics if an IO-APIC page cannot be mapped.
pub fn init() {
    let Some(platform) = acpi::platform() else {
        log::warn!("ioapic: no ACPI topology; external IRQ routing unavailable");
        return;
    };
    let mut regs = IO_APICS.lock();
    for (idx, io) in platform.io_apics.iter().flatten().enumerate() {
        let vaddr = IO_APIC_VADDR_BASE + idx * 0x1000;
        PAGE_MAPPER
            .write()
            .cursor()
            .map(
                VirtAddr::from(vaddr),
                PhysAddr::from((io.paddr & !0xFFF) as usize),
                PageSize::Size4K,
                MappingFlags::READ | MappingFlags::WRITE,
            )
            .expect("ioapic: failed to map MMIO page");
        // Safety: vaddr is the freshly mapped MMIO window.
        let version = unsafe { read_reg(vaddr, 0x01) };
        let redir_count = ((version >> 16) & 0xFF) + 1;
        for pin in 0..redir_count {
            // Safety: pin < redir_count, within the discovered table size.
            unsafe {
                let sel = redirection_sel(pin);
                write_reg(vaddr, sel, read_reg(vaddr, sel) | RINT_MASK);
            }
        }
        log::info!(
            "ioapic[{idx}]: GSI {}..={} @ {:#x} (version {:#x})",
            io.gsi_base,
            io.gsi_base + redir_count - 1,
            io.paddr,
            version & 0xFF
        );
        regs[idx] = Some(IoApicRegs {
            vaddr,
            gsi_base: io.gsi_base,
            redir_count,
        });
    }
}

// --- Routing ----------------------------------------------------------------

fn alloc_vector() -> Option<u8> {
    let vector = NEXT_VECTOR.fetch_add(1, Ordering::AcqRel);
    (vector < EXTERNAL_VECTOR_BASE + EXTERNAL_VECTOR_COUNT as u8).then_some(vector)
}

fn gsi_slot(gsi: u32) -> Option<usize> {
    ROUTED_GSI
        .iter()
        .position(|g| g.load(Ordering::Acquire) == gsi)
}

/// Programs the redirection entry for `gsi` to deliver `vector` to this CPU,
/// honoring the MADT override's polarity/trigger requirements when present.
/// The entry is left unmasked.
fn route(gsi: u32, vector: u8) -> Result<(), ()> {
    let (vaddr, pin) = {
        let regs = IO_APICS.lock();
        let Some(io) = regs
            .iter()
            .flatten()
            .find(|io| gsi >= io.gsi_base && gsi < io.gsi_base + io.redir_count)
        else {
            return Err(());
        };
        (io.vaddr, gsi - io.gsi_base)
    };

    let (active_high, edge) = acpi::platform()
        .and_then(|p| {
            p.overrides
                .iter()
                .flatten()
                .find(|o| o.gsi == gsi)
                .map(|o| (o.active_high, o.edge_triggered))
        })
        .unwrap_or((true, true));

    let mut lower = u32::from(vector) | RINT_MASK; // masked while programming
    if !active_high {
        lower |= RINTPOL_LOW;
    }
    if !edge {
        lower |= REDTRIG_LEVEL;
    }
    let upper = lapic::id() << 24;

    // Safety: vaddr belongs to the covering IO-APIC mapped during init().
    unsafe {
        let sel = redirection_sel(pin);
        write_reg(vaddr, sel, lower); // masked
        write_reg(vaddr, sel + 1, upper);
        write_reg(vaddr, sel, lower & !RINT_MASK); // unmask last
    }
    Ok(())
}

/// Re-masks a routed GSI so its line stops firing.
pub fn mask_gsi(gsi: u32) {
    let (vaddr, pin) = {
        let regs = IO_APICS.lock();
        let Some(io) = regs
            .iter()
            .flatten()
            .find(|io| gsi >= io.gsi_base && gsi < io.gsi_base + io.redir_count)
        else {
            return;
        };
        (io.vaddr, gsi - io.gsi_base)
    };
    // Safety: vaddr belongs to the covering IO-APIC mapped during init().
    unsafe {
        let sel = redirection_sel(pin);
        write_reg(vaddr, sel, read_reg(vaddr, sel) | RINT_MASK);
    }
}

/// Registers `handler` for GSI `gsi`, allocating an external vector and
/// programming its redirection entry. Runs with interrupts disabled.
///
/// # Errors
/// Returns the handler back unchanged when the GSI is already routed or no
/// IO-APIC covers it.
pub fn register_gsi(gsi: u32, handler: HandlerFn) -> Result<(), HandlerFn> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if gsi_slot(gsi).is_some() || alloc_vector_and_route(gsi, handler).is_err() {
            return Err(handler);
        }
        Ok(())
    })
}

fn alloc_vector_and_route(gsi: u32, handler: HandlerFn) -> Result<(), ()> {
    let Some(vector) = alloc_vector() else {
        log::error!("ioapic: external vector space exhausted");
        return Err(());
    };
    let offset = usize::from(vector - EXTERNAL_VECTOR_BASE);
    *HANDLERS[offset].lock() = Some(handler);
    ROUTED_GSI[offset].store(gsi, Ordering::Release);

    match route(gsi, vector) {
        Ok(()) => {
            log::info!("ioapic: GSI {gsi} -> vector {vector:#x}");
            Ok(())
        }
        Err(()) => {
            *HANDLERS[offset].lock() = None;
            ROUTED_GSI[offset].store(u32::MAX, Ordering::Release);
            NEXT_VECTOR.fetch_sub(1, Ordering::AcqRel);
            Err(())
        }
    }
}

// --- Dispatch ---------------------------------------------------------------

fn dispatch_external(offset: usize) {
    let mut ctx = IrqContext { fault_addr: None };
    if let Some(handler) = *HANDLERS[offset].lock() {
        handler(&mut ctx);
    } else {
        log::warn!(
            "ioapic: spurious vector {:#x}",
            EXTERNAL_VECTOR_BASE as usize + offset
        );
    }
    lapic::eoi();
}

/// Generates one `extern "x86-interrupt"` trampoline per external-vector
/// slot plus an [`install`] helper wiring them into the IDT.
macro_rules! external_trampolines {
    ($($name:ident = $offset:literal),* $(,)?) => {
        $(
            extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
                dispatch_external($offset);
            }
        )*
        pub(crate) fn install(idt: &mut x86_64::structures::idt::InterruptDescriptorTable) {
            $(
                idt[EXTERNAL_VECTOR_BASE + $offset].set_handler_fn($name);
            )*
        }
    };
}

external_trampolines!(
    ext_irq00 = 0x00,
    ext_irq01 = 0x01,
    ext_irq02 = 0x02,
    ext_irq03 = 0x03,
    ext_irq04 = 0x04,
    ext_irq05 = 0x05,
    ext_irq06 = 0x06,
    ext_irq07 = 0x07,
    ext_irq08 = 0x08,
    ext_irq09 = 0x09,
    ext_irq0a = 0x0A,
    ext_irq0b = 0x0B,
    ext_irq0c = 0x0C,
    ext_irq0d = 0x0D,
    ext_irq0e = 0x0E,
    ext_irq0f = 0x0F,
    ext_irq10 = 0x10,
    ext_irq11 = 0x11,
    ext_irq12 = 0x12,
    ext_irq13 = 0x13,
    ext_irq14 = 0x14,
    ext_irq15 = 0x15,
    ext_irq16 = 0x16,
    ext_irq17 = 0x17,
    ext_irq18 = 0x18,
    ext_irq19 = 0x19,
    ext_irq1a = 0x1A,
    ext_irq1b = 0x1B,
    ext_irq1c = 0x1C,
    ext_irq1d = 0x1D,
    ext_irq1e = 0x1E,
    ext_irq1f = 0x1F,
);

// --- MSI / MSI-X ------------------------------------------------------------

/// Composes an MSI address word: fixed delivery, physical destination mode,
/// aimed at this CPU's LAPIC.
#[must_use]
pub fn msi_address() -> u32 {
    (0xFEE0_0000u32) | (lapic::id() << 12)
}

/// Composes an MSI data word: fixed delivery, edge-triggered, `vector`.
#[must_use]
pub fn msi_data(vector: u8) -> u32 {
    u32::from(vector)
}

/// Address/data pair for one MSI-X table entry.
#[must_use]
pub fn msix_message(vector: u8) -> (u32, u32) {
    (msi_address(), msi_data(vector))
}

// --- External-line delivery selftest ----------------------------------------

static PROOF_GSI: AtomicU32 = AtomicU32::new(u32::MAX);
static PROOF_TOP: AtomicU64 = AtomicU64::new(0);
static PROOF_BOTTOM: AtomicU64 = AtomicU64::new(0);
static PROOF_DONE: AtomicU8 = AtomicU8::new(0);

fn proof_top_half(_ctx: &mut IrqContext) {
    PROOF_TOP.fetch_add(1, Ordering::AcqRel);
    if !amir_hal::bottom_half::enqueue(proof_bottom_half) {
        log::warn!("ioapic selftest: bottom-half ring full");
    }
}

fn proof_bottom_half() {
    let bottom = PROOF_BOTTOM.fetch_add(1, Ordering::AcqRel) + 1;
    // Five full round-trips prove the line, dispatcher, and queue all work;
    // then silence the PIT so it does not add noise to later milestones.
    if bottom == 5 && PROOF_DONE.swap(1, Ordering::AcqRel) == 0 {
        mask_gsi(PROOF_GSI.load(Ordering::Acquire));
        log::info!(
            "ioapic selftest passed: gsi={}, top={}, bottom={}",
            PROOF_GSI.load(Ordering::Acquire),
            PROOF_TOP.load(Ordering::Acquire),
            bottom
        );
    }
}

/// Routes PIT channel 0 through the IO-APIC as an end-to-end proof of
/// external-line delivery: ISA line 0 -> GSI (MADT override or identity) ->
/// redirection entry -> handler -> bottom-half queue. The line is masked
/// again once the proof completes; the LAPIC/TSC timer is unaffected.
pub fn run_tick_selftest() {
    let Some(platform) = acpi::platform() else {
        log::warn!("ioapic selftest skipped: no ACPI topology");
        return;
    };
    if platform.io_apic_count == 0 {
        return; // PIC-only platform: no redirection entries to prove.
    }
    let gsi = platform
        .overrides
        .iter()
        .flatten()
        .find(|o| o.isa_irq == 0)
        .map_or(0, |o| o.gsi);
    PROOF_GSI.store(gsi, Ordering::Release);

    match register_gsi(gsi, proof_top_half) {
        Ok(()) => {
            log::info!("ioapic selftest: routing PIT ticks on GSI {gsi}");
            pit::start_channel0_periodic(100);
        }
        Err(_) => log::warn!("ioapic selftest: could not route GSI {gsi}"),
    }
}
