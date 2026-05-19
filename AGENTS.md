# AmirOS-rust

Single-crate `#![no_std]` kernel (Rust nightly, edition 2024) targeting x86_64, riscv64, aarch64, loongarch64 via the Limine boot protocol.

## Build & cross-compile

```sh
cargo build --release                                # default: x86_64-unknown-none
cargo build --release --target x86_64-unknown-none
cargo build --release --target riscv64gc-unknown-none-elf
cargo build --release --target aarch64-unknown-none
cargo build --release --target loongarch64-unknown-none
```

Default target and linker flags are set in `.cargo/config.toml`. `build-std` for `core`, `compiler_builtins`, `alloc` is pre-configured — no manual `-Z build-std` needed.

Requires `git submodule update --init` for the Limine bootloader.

## Lint & format (pre-commit gate)

```sh
cargo fmt --all -- --check
cargo clippy -- -D warnings
```

Both must pass with zero output before any commit. Husky pre-commit/pre-push hooks enforce this. CI runs the same commands.

## Security & compliance

```sh
cargo audit       # CI: push + nightly cron
cargo deny check  # CI: push + nightly cron
```

Advisory `RUSTSEC-2024-0436` (unmaintained `paste` via `riscv` crate) is suppressed in `deny.toml`.

## Architecture

```
src/
  main.rs           entry point, all Limine requests in `.limine_requests` section
  allocator.rs      #[global_allocator]: slab heap at 0x4444_4444_0000 (100 MiB)
  heap.rs           demand-paged physical backing (x86_64 page fault handler)
  serial.rs         UART 16550 (PIO on x86_64, MMIO on other arch)
  memory/
    mod.rs          HHDM + kernel remap, FRAME_ALLOCATOR + PAGE_MAPPER globals
    allocator.rs    FreeList<16>-based physical frame allocator
    paging.rs       AmirOSPagingHandler (PagingHandler impl)
  arch/
    mod.rs          cfg(target_arch) dispatch to per-arch mod.rs
    x86_64/         GDT, IDT (breakpoint, PF, double fault w/ IST), CR3 load
    riscv64/        Sv48 paging, SATP setup
    aarch64/        A64 paging
    loongarch64/    LA64 paging
```

Per-arch modules expose `init()`, `holt()`, `PageTable`, `PageTableEntry`.

## Init flow

`main()` → `serial::init()` → `memory::init()` (frame alloc from memmap, HHDM map all physical, kernel remap) → `arch::init()` (GDT, IDT, load CR3/SATP) → `allocator::init()` (slab heap) → SMP bootstrap.

`holt()` halts: `hlt` (x86_64), `wfi` (riscv64, aarch64), `idle 0` (loongarch64).

## Toolchain quirks

- Rust nightly required (`rust-toolchain.toml`), edition 2024.
- `#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]` in `main.rs`.
- `.cargo/config.toml` sets x86_64 rustflags: `-Tlinker-x86_64.ld`, `--image-base=0xffffffff80000000`, `-no-pie`, `relocation-model=static`, `code-model=kernel`.
- `panic = "abort"` in both `[profile.dev]` and `[profile.release]`.
- No tests: `test = false, bench = false` on `[[bin]]`.
- `holt()` differs per arch — see above.

## OpenCode workflow

Load all skills from `.opencode/skills/` before making code changes.
