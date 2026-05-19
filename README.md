# AmirOS

[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
![Rust](https://img.shields.io/badge/rust-nightly_2024_edition-orange.svg)
[![CI](https://github.com/anomalyco/AmirOS-rust/actions/workflows/lint.yml/badge.svg)](https://github.com/anomalyco/AmirOS-rust/actions/workflows/lint.yml)

A multi-architecture hobby OS kernel written in Rust. Targets **x86_64**, **riscv64**, **aarch64**, and **loongarch64** using the [Limine](https://github.com/limine-bootloader/limine) boot protocol.

## Features

- Multi-architecture support via unified abstractions
- Limine boot protocol (revision 0) with requests for framebuffer, memory map, HHDM, SMP, ACPI RSDP, SMBIOS, EFI tables, DTB, kernel file/address, and paging mode
- Higher Half Direct Map (HHDM) of all physical memory with identity-mapped low 4 GiB
- Physical memory frame allocator (free-list based, initialized from bootloader memory map)
- Multi-architecture page table management (`page_table_multiarch`)
- Slab heap allocator with on-demand physical page mapping via page faults (x86_64)
- Serial logging via UART 16550 (PIO on x86_64, MMIO on other architectures)
- SMP bootstrap for application processors
- Interrupt handling on x86_64: GDT, IDT (breakpoint, page fault, double fault with IST)
- ACPI, SMBIOS, EFI, and Device Tree Blob support

## Architecture Support

| Arch | Init | Paging | Interrupts | SMP |
|---|---|---|---|---|
| x86_64 | CR3, GDT, IDT | `X64PageTable` (4LVL) | Breakpoint, PF, Double Fault | Yes |
| riscv64 | SATP (Sv48) | `Sv48PageTable` | — | Yes |
| aarch64 | — | `A64PageTable` | — | Yes |
| loongarch64 | — | `LA64PageTable` | — | — |

## Getting Started

### Prerequisites

- Rust nightly — install via `rustup toolchain install nightly`
- Build targets:
  ```sh
  rustup target add x86_64-unknown-none
  rustup target add riscv64gc-unknown-none-elf
  rustup target add aarch64-unknown-none
  rustup target add loongarch64-unknown-none
  ```
- Install `xorriso` and the appropriate `qemu-system-*` for your target
- Initialize the Limine submodule:
  ```sh
  git submodule update --init
  ```

### Build

```sh
cargo build --release
```

Cross-compile for a specific target:

```sh
cargo build --release --target x86_64-unknown-none
cargo build --release --target riscv64gc-unknown-none-elf
cargo build --release --target aarch64-unknown-none
cargo build --release --target loongarch64-unknown-none
```

### Create Bootable ISO

```sh
mkdir -p iso_root/boot
cp target/x86_64-unknown-none/release/amir_os iso_root/boot/
cp limine.conf limine/limine-bios.sys limine/limine-bios-cd.bin \
   limine/limine-uefi-cd.bin iso_root/

xorriso -as mkisofs -b limine-bios-cd.bin \
   -no-emul-boot -boot-load-size 4 -boot-info-table \
   --efi-boot limine-uefi-cd.bin -efi-boot-part \
   --efi-boot-image --protective-msdos-label \
   iso_root -o amir_os.iso

./limine/limine bios-install amir_os.iso
```

### Run in QEMU

```sh
qemu-system-x86_64 -cdrom amir_os.iso -serial stdio
```

For other architectures, use the appropriate QEMU binary:

```sh
qemu-system-riscv64   -cdrom amir_os.iso -serial stdio -machine virt
qemu-system-aarch64   -cdrom amir_os.iso -serial stdio -machine virt
```

## Project Structure

```
├── src/
│   ├── main.rs            — Kernel entry point, Limine requests, SMP bootstrap
│   ├── allocator.rs       — Global allocator (slab heap, 100 MiB at 0x4444_4444_0000)
│   ├── heap.rs            — Heap implementation with on-demand physical page mapping
│   ├── serial.rs          — UART 16550 serial driver and logger
│   ├── arch/
│   │   ├── mod.rs         — Architecture dispatch via cfg attributes
│   │   ├── x86_64/        — GDT, IDT, paging, CR3 loading
│   │   │   ├── gdt.rs     — Global Descriptor Table with TSS
│   │   │   ├── idt.rs     — Interrupt Descriptor Table, demand paging PF handler
│   │   │   └── paging.rs  — X64PageTable type alias
│   │   ├── riscv64/       — SATP setup, Sv48 paging
│   │   │   └── paging.rs  — Sv48PageTable type alias
│   │   ├── aarch64/       — Paging
│   │   │   └── paging.rs  — A64PageTable type alias
│   │   └── loongarch64/   — Paging
│   │       └── paging.rs  — LA64PageTable type alias
│   └── memory/
│       ├── mod.rs         — HHDM + kernel mapping initialization
│       ├── allocator.rs   — Physical frame allocator (free-list)
│       └── paging.rs      — Multi-arch PagingHandler (AmirOSPagingHandler)
├── linker-x86_64.ld       — x86_64 linker script (higher-half, Limine requests PHDR)
├── linker-riscv64.ld      — riscv64 linker script (higher-half)
├── limine.conf            — Limine boot configuration
├── limine/                — Limine bootloader submodule
├── rust-toolchain.toml    — Nightly channel + target specifications
├── .cargo/
│   └── config.toml        — Build flags, linker script, build-std config
├── .github/
│   └── workflows/         — CI: audit, deny, lint, opencode
└── .husky/                — Pre-commit and pre-push hooks
```

## Technical Details

### Boot Process

The kernel uses the Limine boot protocol. On startup, it:

1. Initializes serial logging via UART 16550
2. Validates the bootloader supports base revision
3. Requests and stores bootloader information, memory map, framebuffer, and other system tables
4. Initializes the physical memory frame allocator from the memory map
5. Maps all physical memory into the higher half (HHDM) and identity-maps the low 4 GiB
6. Remaps the kernel at its higher-half virtual address
7. Performs architecture-specific initialization (GDT, IDT, CR3, SATP, etc.)
8. Initializes the slab heap allocator
9. Bootstraps application processors (SMP)

### Memory Management

- **Frame Allocator**: A `FreeList<16>`-based allocator that tracks usable physical memory regions from the bootloader's memory map. Supports allocation and deallocation of page-aligned physical frames.
- **Page Tables**: The `page_table_multiarch` crate provides a unified interface across all four architectures. `AmirOSPagingHandler` bridges frame allocation requests to the kernel's frame allocator.
- **HHDM**: All physical memory (excluding bad regions) is mapped at `phys_addr + hhdm_offset` using the largest available page size (1 GiB → 2 MiB → 4 KiB). The low 4 GiB is also identity-mapped to ensure a seamless transition when switching page tables.
- **Kernel Heap**: 100 MiB slab allocator at `0x4444_4444_0000`. On x86_64, physical pages are allocated on demand via the page fault handler — the heap range is mapped lazily as memory is accessed.

### Architecture Abstraction

Each architecture provides a consistent interface:

- `init()` — architecture-specific initialization
- `holt()` — halt the CPU (HLT/WFI/IDLE loop)
- `PageTable` / `PageTableEntry` — page table type aliases

Conditional compilation (`#[cfg(target_arch = "...")]`) in `src/arch/mod.rs` selects the correct backend at build time.

### Kernel Heap & Demand Paging (x86_64)

The slab heap allocator (`slab_allocator_rs`) lives at a fixed virtual address. When `SlabHeap::new()` writes its intrusive free-list metadata, the writes trigger page faults. The x86_64 page fault handler detects addresses in the heap range, allocates a physical frame, and maps it — allowing the heap to use physical memory proportional to actual usage rather than pre-allocating 100 MiB.

## CI/CD & Quality

| Workflow | Trigger | Action |
|---|---|---|
| **Lint** | Push/PR to main | `cargo fmt --check` + `cargo clippy -D warnings` |
| **Security Audit** | Push/PR, nightly cron | `cargo audit` |
| **Cargo Deny** | Push/PR, nightly cron | License, ban, and source checks |
| **Dependabot** | Daily | Crate and toolchain dependency updates |

Pre-commit and pre-push hooks run `cargo fmt` and `cargo clippy` via Husky.

## License

Dual-licensed under either:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
