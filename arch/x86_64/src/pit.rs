//! 8253/8254 PIT channel 2 used solely as a calibration stopwatch.
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
fn ch2_data_port() -> Port<u8> {
    Port::new(0x42)
}
fn ctrl_port() -> Port<u8> {
    Port::new(0x61)
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
