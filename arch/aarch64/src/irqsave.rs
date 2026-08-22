//! Interrupt-saving primitives (see `arch/x86_64/src/irqsave.rs` for the
//! guard types layered on top by consumers).
//!
//! aarch64: manipulates `DAIF.I` (interrupt mask bit 7).

/// Disables IRQs (`DAIF.I` set), returning whether they were enabled.
#[must_use = "pass the returned flag to restore()"]
pub fn save_disable() -> bool {
    let prev: u64;
    // Safety: reads DAIF and sets the I mask bit only.
    unsafe {
        core::arch::asm!(
            "mrs {prev}, daif",
            "msr daifset, #2",
            prev = out(reg) prev,
            options(nomem)
        );
    }
    prev & (1 << 7) != 0
}

/// Restores the interrupt state captured by a prior [`save_disable`].
pub fn restore(were_enabled: bool) {
    if were_enabled {
        // Safety: clears the I mask bit only.
        unsafe {
            core::arch::asm!("msr daifclr, #2", options(nomem));
        }
    }
}
