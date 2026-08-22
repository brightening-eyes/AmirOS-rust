# AmirOS-rust

Workspace-based `#![no_std]` kernel (Rust nightly, edition 2024) targeting x86_64, riscv64, aarch64, loongarch64 via the Limine boot protocol.

## Build & cross-compile

```sh
cargo build --release                                # default: x86_64-unknown-none
cargo build --release --target x86_64-unknown-none
cargo build --release --target riscv64gc-unknown-none-elf
cargo build --release --target aarch64-unknown-none
cargo build --release --target loongarch64-unknown-none
```

Default target and linker flags are set in `.cargo/config.toml`. `build-std` for `core`, `compiler_builtins`, `alloc` is pre-configured — no manual `-Z build-std` needed.

The workspace uses `default-members = ["kernel"]`: bare commands build only the kernel for the active target, which pulls exactly one per-arch crate via target-specific dependencies. Never use `--workspace` for builds/clippy; verify other targets with explicit `-p amir_kernel --target <triple>`.

Requires `git submodule update --init` for the Limine bootloader.

## Lint & format (pre-commit gate)

```sh
cargo fmt --all -- --check
cargo clippy -- -D warnings
```

Both must pass with zero output before any commit. Husky pre-commit/pre-push hooks enforce this. CI runs the same commands.

## QEMU smoke test

```sh
cargo build --release
bash scripts/qemu-smoke.sh                 # Linux/macOS (CI: qemu-smoke.yml)
powershell -File scripts/qemu-smoke.ps1    # Windows local
```

Boots a headless q35 VM from a freshly assembled Limine ISO and asserts the serial log contains `allocator initialized`. Needs `xorriso` + `qemu-system-x86_64` on PATH; limine host tool comes from `limine/limine.c` (Linux) or `limine/limine.exe` (Windows).

## Security & compliance

```sh
cargo audit       # CI: push + nightly cron
cargo deny check  # CI: push + nightly cron
```

Advisory `RUSTSEC-2024-0436` (unmaintained `paste` via `riscv` crate) is suppressed in `deny.toml`.

## Architecture

```
kernel/src/main.rs  entry point, all Limine requests in `.limine_requests` section
hal/                arch-neutral traits + `&'static dyn` registries (irq, timer, percpu, uart, framebuffer)
mm/                 FRAME_ALLOCATOR + PAGE_MAPPER globals, HHDM + kernel remap,
                    FreeList<16> frame allocator, AmirOSPagingHandler, cfg-selected
                    PageTable/PageTableEntry aliases, #[global_allocator] slab heap
                    at 0x4444_4444_0000 (100 MiB, demand-paged via x86_64 PF handler)
drivers/            serial: UART 16550 (PIO on x86_64, MMIO on other arch) + logger
arch/               facade crate: cfg(target_arch) re-export of exactly one backend
arch/x86_64/        GDT, IDT (breakpoint, PF, double fault w/ IST), CR3 load
arch/riscv64/       Sv48 paging, SATP setup
arch/aarch64/       A64 paging
arch/loongarch64/   LA64 paging
```

Dependency direction: `kernel → {drivers, mm, arch}`; `{drivers, arch/x86_64..} → mm`; `mm` owns all paging types (no mm↔arch cycle). Per-arch crates expose `init()`, `holt()`.

## Init flow

`main()` → `serial::init()` → `memory::init()` (frame alloc from memmap, HHDM map all physical, kernel remap) → `arch::init()` (GDT, IDT, load CR3/SATP) → `allocator::init()` (slab heap) → SMP bootstrap.

`holt()` halts: `hlt` (x86_64), `wfi` (riscv64, aarch64), `idle 0` (loongarch64).

## Toolchain quirks

- Rust nightly required (`rust-toolchain.toml`), edition 2024.
- `#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]` lives in `arch/x86_64/src/lib.rs`.
- `.cargo/config.toml` sets x86_64 rustflags: `-Tlinker-x86_64.ld`, `--image-base=0xffffffff80000000`, `-no-pie`, `relocation-model=static`, `code-model=kernel`.
- `panic = "abort"` in both `[profile.dev]` and `[profile.release]` (workspace root).
- No tests: `test = false, bench = false` on `[[bin]] amir_os` in `kernel/Cargo.toml`.
- aarch64/loongarch64 link without a dedicated linker script → lld warns about `_start`; pre-existing, Limine uses the ELF entry anyway.
- `holt()` differs per arch — see above.

## OpenCode workflow

Load all skills from `.opencode/skills/` before making code changes.
