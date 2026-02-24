//! This module contains architecture-specific code.
//!
//! It is responsible for initializing the CPU, handling interrupts, and other
//! low-level, architecture-dependent tasks.

#![allow(clippy::module_inception)]

// Use conditional compilation to include the correct submodule.
#[cfg(target_arch = "x86_64")]
#[path = "x86_64/mod.rs"]
mod arch;

#[cfg(target_arch = "riscv64")]
#[path = "riscv64/mod.rs"]
mod arch;

#[cfg(target_arch = "loongarch64")]
#[path = "loongarch64/mod.rs"]
mod arch;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64/mod.rs"]
mod arch;

pub use arch::*;
