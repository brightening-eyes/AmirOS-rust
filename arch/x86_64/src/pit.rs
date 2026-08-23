//! 8253/8254 PIT: channel 2 as a calibration stopwatch, channel 0 as a
//! periodic external-line source for interrupt-controller selftests.
//!
//! `start_window_ms` programs a one-shot window; `await_window_end` spins
//! until channel 2's OUT line rises. Sample your own clocks between the two
//! calls to measure elapsed time against known hardware.

use core::arch::asm;
use x86_64::instructions::port::Port;

/// Base PIT oscillator frequency.
const PIT_FREQUENCY_HZ: u64 = 1_193_182;

fn cmd_port() -> Port<u8> {
    Port::new(0x43)
}
fn ch0_data_port() -> Port<u8> {
    Port::new(0x40)
}
fn ch2_data_port() -> Port<u8> {
    Port::new(0x42)
}
fn ctrl_port() -> Port<u8> {
    Port::new(0x61)
}

/// Programs channel 0 as a rate generator pulsing its OUT line `hz` times
/// per second — the classic IRQ0 source for routing through a controller.
///
/// The output runs until reprogrammed; silencing delivery is the interrupt
/// controller's job (mask the routed IO-APIC entry or PIC line).
pub fn start_channel0_periodic(hz: u64) {
    let divisor = (PIT_FREQUENCY_HZ / hz.max(1)).clamp(1, u64::from(u16::MAX)) as u16;
    unsafe {
        // Channel 0 | access lo+hi | mode 2 (rate generator) | binary.
        cmd_port().write(0b0011_0100);
        ch0_data_port().write((divisor & 0xFF) as u8);
        ch0_data_port().write((divisor >> 8) as u8);
    }
}

/// Arms channel 2 in one-shot (interrupt-on-terminal-count) mode for ~`ms`.
pub fn start_window_ms(ms: u64) {
    let count = ((PIT_FREQUENCY_HZ * ms) / 1000).clamp(1, 0xFFFF) as u16;
    unsafe {
        // Gate off, speaker off while programming.
        let ctrl = ctrl_port().read();
        ctrl_port().write(ctrl & !0b11);
        // Channel 2 | access lo+hi | mode 0 (one-shot) | binary.
        cmd_port().write(0b1011_0000);
        ch2_data_port().write((count & 0xFF) as u8);
        ch2_data_port().write((count >> 8) as u8);
        // Open the gate: the countdown starts now.
        ctrl_port().write((ctrl & !0b10) | 0b01);
    }
}

/// Spins until the programmed window elapses (channel-2 OUT goes high).
///
/// Bounded by an iteration cap so a dead PIT cannot hang boot forever.
pub fn await_window_end() {
    const SPIN_CAP: u64 = 500_000_000;
    let mut spins = 0u64;
    while unsafe { ctrl_port().read() } & 0x20 == 0 {
        spins += 1;
        if spins >= SPIN_CAP {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Reads the current time-stamp counter.
pub fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    (u64::from(hi) << 32) | u64::from(lo)
}
