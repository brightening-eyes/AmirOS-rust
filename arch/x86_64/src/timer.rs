//! Timer bring-up: CPUID feature detection, PIT-based calibration, the tick
//! handler, and the `hal::Timer` implementation.
//!
//! Mode selection:
//! - TSC-deadline (absolute one-shot via `IA32_TSC_DEADLINE`) when CPUID
//!   advertises it
//! - otherwise LAPIC one-shot mode, re-armed by software every tick
//!
//! Both paths are calibrated once against a 10 ms PIT channel-2 window.

use crate::{lapic, pit};
use amir_hal::{Timer, timer as hal_timer};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::registers::model_specific::Msr;

/// `IA32_TSC_DEADLINE` — not exposed by the x86_64 crate.
fn tsc_deadline_msr() -> Msr {
    Msr::new(0x6B0)
}

/// LAPIC timer tick vector. Sits above the remapped PIC range (`0x20..0x2F`)
/// so the legacy controller can own its full band without colliding with
/// kernel-internal vectors; the external device band starts at `0x40`.
pub const TIMER_VECTOR: u8 = 0x30;
/// Target tick rate. 100 Hz keeps early logs readable and CI waits short.
const TICK_HZ: u64 = 100;
const NS_PER_TICK: u64 = 1_000_000_000 / TICK_HZ;

static TICKS: AtomicU64 = AtomicU64::new(0);
static USING_TSC_DEADLINE: AtomicBool = AtomicBool::new(false);
static TSC_HZ: AtomicU64 = AtomicU64::new(0);
static LAPIC_TICKS_PER_PERIOD: AtomicU64 = AtomicU64::new(0);
static TSC_AT_BOOT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TimerCaps {
    tsc_deadline: bool,
    invariant_tsc: bool,
}

fn detect_caps() -> TimerCaps {
    let cpuid = raw_cpuid::CpuId::new();
    let tsc_deadline = cpuid
        .get_feature_info()
        .map(|f| f.has_tsc_deadline())
        .unwrap_or(false);
    // Leaf 0x8000_0007 (APM info) reports INVTSC; the getter is None below it.
    let invariant_tsc = cpuid
        .get_advanced_power_mgmt_info()
        .map(|e| e.has_invariant_tsc())
        .unwrap_or(false);
    log::info!("timer caps: tsc_deadline={tsc_deadline}, invariant_tsc={invariant_tsc}");
    TimerCaps {
        tsc_deadline,
        invariant_tsc,
    }
}

/// Samples TSC and LAPIC countdown across one PIT window to derive rates.
fn calibrate() -> (u64, u64) {
    lapic::arm_oneshot(TIMER_VECTOR, u32::MAX);

    pit::start_window_ms(10);
    let tsc_start = pit::read_tsc();
    let lapic_start = lapic::current_count();
    pit::await_window_end();
    let tsc_end = pit::read_tsc();
    let lapic_end = lapic::current_count();

    let tsc_hz = (tsc_end - tsc_start).saturating_mul(100).max(1);
    let lapic_ticks_per_10ms = lapic_start.wrapping_sub(lapic_end);
    let lapic_hz = u64::from(lapic_ticks_per_10ms).saturating_mul(100).max(1);
    (tsc_hz, lapic_hz)
}

/// Full timer bring-up. Runs after [`lapic::map`] with interrupts disabled.
pub fn init() {
    let caps = detect_caps();
    let (tsc_hz, lapic_hz) = calibrate();
    log::info!("calibrated: TSC {tsc_hz} Hz, LAPIC timer {lapic_hz} Hz (div16)");

    TSC_HZ.store(tsc_hz, Ordering::Release);
    LAPIC_TICKS_PER_PERIOD.store(lapic_hz / TICK_HZ, Ordering::Release);
    TSC_AT_BOOT.store(pit::read_tsc(), Ordering::Relaxed);
    USING_TSC_DEADLINE.store(caps.tsc_deadline && caps.invariant_tsc, Ordering::Release);

    hal_timer::register(&X86_TIMER);
    schedule_next_period();
    x86_64::instructions::interrupts::enable();
    log::info!(
        "timer started: {TICK_HZ} Hz on vector {TIMER_VECTOR:#x} (tsc-deadline={})",
        USING_TSC_DEADLINE.load(Ordering::Acquire)
    );
}

/// Nanoseconds since timer init (TSC-backed when invariant, else tick count).
pub fn now_ns() -> u64 {
    let tsc_hz = TSC_HZ.load(Ordering::Acquire);
    if tsc_hz > 0 {
        let cycles = pit::read_tsc().wrapping_sub(TSC_AT_BOOT.load(Ordering::Relaxed));
        u64::try_from(u128::from(cycles) * 1_000_000_000 / u128::from(tsc_hz)).unwrap_or(u64::MAX)
    } else {
        TICKS.load(Ordering::Acquire) * NS_PER_TICK
    }
}

/// Arms the next periodic tick using whichever mode is available.
fn schedule_next_period() {
    if USING_TSC_DEADLINE.load(Ordering::Acquire) {
        let deadline_cycles = u128::from(now_ns() + NS_PER_TICK)
            * u128::from(TSC_HZ.load(Ordering::Acquire))
            / 1_000_000_000;
        // Safety: MSR write; value derived from calibrated TSC rate.
        unsafe {
            tsc_deadline_msr().write(u64::try_from(deadline_cycles).unwrap_or(u64::MAX));
        }
        return;
    }
    let period = LAPIC_TICKS_PER_PERIOD.load(Ordering::Acquire);
    if period > 0 && period <= u64::from(u32::MAX) {
        lapic::arm_oneshot(TIMER_VECTOR, period as u32);
    }
}

pub(crate) extern "x86-interrupt" fn tick_handler(
    _frame: x86_64::structures::idt::InterruptStackFrame,
) {
    let ticks = TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    if ticks <= 20 || ticks.is_multiple_of(TICK_HZ) {
        log::info!("timer: ticks={ticks}");
    }
    lapic::eoi();
    schedule_next_period();
}

struct X86Timer;

impl Timer for X86Timer {
    fn now_ns(&self) -> u64 {
        now_ns()
    }

    fn set_oneshot(&self, deadline_ns: u64) {
        if USING_TSC_DEADLINE.load(Ordering::Acquire) {
            let cycles = u128::from(deadline_ns) * u128::from(TSC_HZ.load(Ordering::Acquire))
                / 1_000_000_000;
            // Safety: MSR write; value derived from calibrated TSC rate.
            unsafe {
                tsc_deadline_msr().write(u64::try_from(cycles).unwrap_or(u64::MAX));
            }
        }
        // Without TSC-deadline there is no absolute one-shot yet; the periodic
        // tick stays active and consumers can poll `now_ns`.
    }
}

static X86_TIMER: X86Timer = X86Timer;
