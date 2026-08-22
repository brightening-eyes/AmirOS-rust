//! Architecture facade crate.
//!
//! Re-exports the implementation of exactly one per-architecture crate,
//! selected at compile time by `target_arch`. Arch-neutral code (the kernel)
//! depends only on this crate and calls `amir_arch::init()`, `holt()`, etc.

#![no_std]

#[cfg(target_arch = "aarch64")]
pub use amir_arch_aarch64 as imp;
#[cfg(target_arch = "loongarch64")]
pub use amir_arch_loongarch64 as imp;
#[cfg(target_arch = "riscv64")]
pub use amir_arch_riscv64 as imp;
#[cfg(target_arch = "x86_64")]
pub use amir_arch_x86_64 as imp;

pub use imp::*;
