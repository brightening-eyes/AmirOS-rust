//! Interrupt-saving primitives.
//!
//! TODO(loongarch): implement against `CRMD.IE` (CSR `0x0`, bit 2) via
//! `csrxchg`. Nothing enables interrupts on this target yet, so the stubs
//! below panic rather than silently corrupting interrupt state.

/// Not implemented on loongarch64 yet.
///
/// # Panics
/// Always — no consumer exists on this target until interrupt bring-up lands.
#[must_use = "pass the returned flag to restore()"]
pub fn save_disable() -> bool {
    unimplemented!("loongarch64 irqsave: not implemented until interrupt bring-up")
}

/// Not implemented on loongarch64 yet.
///
/// # Panics
/// Always — see [`save_disable`].
pub fn restore(_were_enabled: bool) {
    unimplemented!("loongarch64 irqsave: not implemented until interrupt bring-up")
}
