//! This module contains architecture-specific code.
//!
//! It is responsible for initializing the CPU, handling interrupts, and other
//! low-level, architecture-dependent tasks.

// Use conditional compilation to include the correct submodule.
#[cfg(target_arch = "x86_64")]
#[path = "x86_64/mod.rs"]
pub mod arch;

#[cfg(target_arch = "riscv64")]
#[path = "riscv64/mod.rs"]
pub mod arch;

#[cfg(target_arch="loongarch64")]
#[path="loongarch64/mod.rs"]
pub mod arch;

#[cfg(target_arch="aarch64")]
#[path="aarch64/mod.rs"]
pub mod arch;

/// Initializes architecture-specific features.
///
/// This function should be called once at the beginning of the kernel's execution.
/// It will delegate to the appropriate architecture's init function.
pub fn init()
{
    arch::init();
}

pub fn holt()
{
arch::holt();
}
