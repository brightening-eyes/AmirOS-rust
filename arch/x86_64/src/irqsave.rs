//! Interrupt-saving synchronization primitives (the direct cfg path of the
//! HAL: raw register manipulation that cannot be an object-safe trait).
//!
//! [`save_disable`] / [`restore`] nest correctly: restore only re-enables
//! when the matching save observed enabled interrupts.
//! [`IrqSaveMutex`] wraps a spin lock so critical sections run with IRQs off.

use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use spin::Mutex as SpinMutex;

/// Disables interrupts, returning whether they were enabled before.
#[must_use = "pass the returned flag to restore()"]
pub fn save_disable() -> bool {
    let were_enabled = x86_64::instructions::interrupts::are_enabled();
    x86_64::instructions::interrupts::disable();
    were_enabled
}

/// Restores the interrupt state captured by a prior [`save_disable`].
pub fn restore(were_enabled: bool) {
    if were_enabled {
        x86_64::instructions::interrupts::enable();
    }
}

/// Lock that keeps interrupts disabled for the duration of the borrow.
///
/// Locking spins with interrupts off, so interrupt handlers on the same CPU
/// cannot wedge the holder. The guard releases the spin lock *before*
/// restoring the saved IF state.
pub struct IrqSaveMutex<T> {
    inner: SpinMutex<T>,
}

impl<T> IrqSaveMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: SpinMutex::new(value),
        }
    }

    /// Locks with interrupts disabled; both are released on guard drop.
    pub fn lock(&self) -> IrqSaveGuard<'_, T> {
        let were_enabled = save_disable();
        IrqSaveGuard {
            // Safety: dropped exactly once by the guard's Drop impl.
            inner: ManuallyDrop::new(self.inner.lock()),
            were_enabled,
        }
    }
}

pub struct IrqSaveGuard<'a, T> {
    inner: ManuallyDrop<SpinMutexGuard<'a, T>>,
    were_enabled: bool,
}

type SpinMutexGuard<'a, T> = spin::mutex::MutexGuard<'a, T, spin::relax::Spin>;

impl<T> Deref for IrqSaveGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for IrqSaveGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Drop for IrqSaveGuard<'_, T> {
    fn drop(&mut self) {
        // Safety: created exactly once in [`IrqSaveMutex::lock`]; the guard
        // is consumed here and nowhere else.
        unsafe { ManuallyDrop::drop(&mut self.inner) };
        restore(self.were_enabled);
    }
}
