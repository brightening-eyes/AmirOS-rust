//! Local APIC driver: MMIO mapping, enable, EOI, and timer register access.
//!
//! The LAPIC base address comes from the MADT (falling back to the
//! architectural default). Its MMIO page is mapped at a fixed high-half
//! scratch virtual address through the kernel page mapper.

use amir_mm::PAGE_MAPPER;
use core::sync::atomic::{AtomicUsize, Ordering};
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};

/// Fixed virtual address for the LAPIC MMIO page. High canonical half,
/// below the kernel image base, unused by anything else.
const LAPIC_VADDR: usize = 0xFFFF_FF80_0000_0000;

/// Architectural default LAPIC base used when the MADT does not override it.
pub const DEFAULT_LAPIC_PADDR: usize = 0xFEE0_0000;

static LAPIC_PADDR: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn reg(offset: u32) -> *mut u32 {
    (LAPIC_VADDR + offset as usize) as *mut u32
}

#[inline]
fn read(offset: u32) -> u32 {
    unsafe { reg(offset).read_volatile() }
}

#[inline]
fn write(offset: u32, value: u32) {
    unsafe { reg(offset).write_volatile(value) }
}

/// Maps the LAPIC MMIO page. Must run before any other function here.
///
/// # Panics
/// Panics if the mapping fails.
pub fn map(paddr: usize) {
    let aligned = paddr & !0xFFF;
    if aligned != DEFAULT_LAPIC_PADDR {
        log::warn!("lapic: non-default base {paddr:#x}");
    }
    PAGE_MAPPER
        .write()
        .cursor()
        .map(
            VirtAddr::from(LAPIC_VADDR),
            PhysAddr::from(aligned),
            PageSize::Size4K,
            MappingFlags::READ | MappingFlags::WRITE,
        )
        .expect("lapic: failed to map MMIO page");
    LAPIC_PADDR.store(aligned | 1, Ordering::Release);
}

/// Returns `true` once [`map`] has run.
pub fn mapped() -> bool {
    LAPIC_PADDR.load(Ordering::Acquire) & 1 == 1
}

fn assert_mapped() {
    assert!(mapped(), "lapic: accessed before mapping");
}

/// The APIC ID of the calling CPU.
pub fn id() -> u32 {
    assert_mapped();
    read(0x20) >> 24
}

/// Enables the LAPIC and points spurious interrupts at `spurious_vector`.
pub fn enable(spurious_vector: u8) {
    assert_mapped();
    const SPURIOUS_ENABLE_BIT: u32 = 1 << 8;
    let spiv = read(0xF0);
    write(
        0xF0,
        spiv | SPURIOUS_ENABLE_BIT | u32::from(spurious_vector),
    );
    log::info!("lapic enabled (spurious vector {spurious_vector:#x})");
}

/// Signals end-of-interrupt for the highest-priority handled line.
pub fn eoi() {
    assert_mapped();
    write(0xB0, 0);
}

// --- Timer registers -------------------------------------------------------

const LVT_TIMER: u32 = 0x320;
const INITIAL_COUNT: u32 = 0x380;
const DIVIDE_CONFIG: u32 = 0x3E0;
/// Divide configuration value for divide-by-16.
const DIVIDE_BY_16: u32 = 0b0011;
/// LVT bits: mask bit (16), one-shot mode (17).
const LVT_MASKED: u32 = 1 << 16;

/// Arms the LAPIC timer in one-shot mode to fire `vector` after `ticks`.
pub fn arm_oneshot(vector: u8, ticks: u32) {
    assert_mapped();
    write(DIVIDE_CONFIG, DIVIDE_BY_16);
    write(INITIAL_COUNT, 0); // stop while reprogramming
    write(LVT_TIMER, u32::from(vector)); // one-shot, not masked
    write(INITIAL_COUNT, ticks);
}

/// Stops the LAPIC timer.
pub fn disarm_timer() {
    assert_mapped();
    write(INITIAL_COUNT, 0);
    write(LVT_TIMER, LVT_MASKED);
}

/// Reads the current (decrementing) timer count. Requires the timer to be
/// running; returns 0 when it has expired or is stopped.
pub fn current_count() -> u32 {
    assert_mapped();
    read(INITIAL_COUNT)
}
