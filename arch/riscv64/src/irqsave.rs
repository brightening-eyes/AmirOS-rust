//! Interrupt-saving primitives (see `arch/x86_64/src/irqsave.rs` for the
//! guard types layered on top by consumers).
//!
//! riscv64: manipulates `sstatus.SIE` (CSR `0x100`, bit 1).

/// Disables supervisor interrupts, returning whether they were enabled.
#[must_use = "pass the returned flag to restore()"]
pub fn save_disable() -> bool {
    let prev: usize;
    // Safety: CSR read-modify-clear of the SIE bit only.
    unsafe {
        core::arch::asm!("csrrci {prev}, 0x100, 2", prev = out(reg) prev);
    }
    prev & 0b10 != 0
}

/// Restores the interrupt state captured by a prior [`save_disable`].
pub fn restore(were_enabled: bool) {
    if were_enabled {
        // Safety: sets SIE; destination register is discarded.
        unsafe {
            core::arch::asm!("csrrsi x0, 0x100, 2");
        }
    }
}
