# AmirOS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/brightening-eyes/AmirOS-rust/blob/main/LICENSE)
[![Lint](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/lint.yml/badge.svg)](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/lint.yml)
[![Security Audit](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/audit.yml/badge.svg)](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/audit.yml)
[![Cargo Deny](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/deny.yml/badge.svg)](https://github.com/brightening-eyes/AmirOS-rust/actions/workflows/deny.yml)

An experimental operating system kernel written in Rust, targeting multiple CPU architectures with modern memory management and SMP support.

## Features

- **Multi-Architecture Support** - Runs on x86_64, RISC-V 64, AArch64, and LoongArch64
- **Limine Boot Protocol** - Modern bootloader with advanced features
- **Higher Half Kernel** - Kernel mapped at `0xffffffff80000000` for user space below
- **Advanced Memory Management**
  - Frame allocator using free-list
  - Higher Half Direct Mapping (HHDM) for all physical memory
  - On-demand heap page mapping
  - Support for 4KB, 2MB, and 1GB pages
- **Heap Allocator** - Slab-based allocator with automatic page mapping
- **SMP Support** - Multiprocessor boot via Limine MP protocol
- **Serial Console** - UART 16550 driver with `log` crate integration
- **x86_64 Interrupts** - GDT, TSS, and IDT with exception handlers

## Supported Architectures

| Architecture | Target | Status | Notes |
|--------------|--------|--------|-------|
| **x86_64** | `x86_64-unknown-none` | Most Complete | GDT, IDT, full interrupt support |
| **RISC-V 64** | `riscv64gc-unknown-none-elf` | Partial | Sv48 paging |
| **AArch64** | `aarch64-unknown-none` | Partial | Basic initialization |
| **LoongArch64** | `loongarch64-unknown-none` | Minimal | LA64 paging |

## Prerequisites

- **Rust Nightly** - This project uses unstable features
- **QEMU** - For running the kernel in a virtual machine
- **Limine** - Bootloader (included in `limine/` directory)

### Installing Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

### Installing QEMU

**Linux (Debian/Ubuntu):**
```bash
sudo apt install qemu-system-x86
```

**Linux (Arch):**
```bash
sudo pacman -S qemu-system-x86
```

**macOS:**
```bash
brew install qemu
```

**Windows:**
Download from [qemu.org](https://www.qemu.org/download/)

## Building

```bash
# Clone the repository
git clone https://github.com/brightening-eyes/AmirOS-rust.git
cd AmirOS-rust

# Build for x86_64 (default)
cargo build

# Build for other architectures
cargo build --target riscv64gc-unknown-none-elf
cargo build --target aarch64-unknown-none
cargo build --target loongarch64-unknown-none

# Build in release mode
cargo build --release
```

## Running

### With QEMU (x86_64)

```bash
# Build and create bootable image
cargo build

# Run with QEMU
qemu-system-x86_64 \
  -cdrom target/amir_os/amir_os-x86_64.iso \
  -serial stdio \
  -m 512M
```

### QEMU Options

| Option | Description |
|--------|-------------|
| `-cdrom` | Boot from ISO image |
| `-serial stdio` | Redirect serial output to console |
| `-m 512M` | Allocate 512MB RAM |
| `-nographic` | Disable graphical output |
| `-device isa-debug-exit,iobase=0xf4,iosize=0x04` | Enable test exit device |

## Project Structure

```
AmirOS-rust/
├── src/
│   ├── main.rs              # Entry point, Limine requests, panic handler
│   ├── allocator.rs         # Global heap allocator setup
│   ├── heap.rs              # Slab allocator with on-demand mapping
│   ├── serial.rs            # UART 16550 driver + logging
│   ├── memory/
│   │   ├── mod.rs           # Memory manager, HHDM setup
│   │   ├── allocator.rs     # Frame allocator
│   │   └── paging.rs        # Multiarch paging handler
│   └── arch/
│       ├── mod.rs           # Architecture dispatcher
│       ├── x86_64/          # x86_64 specific (GDT, IDT, paging)
│       ├── riscv64/         # RISC-V specific (SATP, paging)
│       ├── aarch64/         # ARM64 specific
│       └── loongarch64/     # LoongArch64 specific
├── limine/                  # Limine bootloader binaries
├── limine.conf              # Bootloader configuration
├── linker-x86_64.ld         # x86_64 linker script
├── linker-riscv64.ld        # RISC-V linker script
├── Cargo.toml               # Package configuration
├── rust-toolchain.toml      # Rust toolchain specification
├── deny.toml                # cargo-deny configuration
└── .cargo/
    └── config.toml          # Build configuration
```

## Architecture Overview

### Memory Layout

```
Virtual Address Space (x86_64):
┌─────────────────────────────┐ 0xffffffffffffffff
│        Kernel Space         │
│  ┌───────────────────────┐  │ 0xffffffff80000000
│  │   Kernel Image        │  │ - Higher half kernel
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │   Heap Allocator      │  │ 0x444444440000
│  └───────────────────────┘  │
├─────────────────────────────┤ 0x7ffffffffff
│        User Space           │
│  (Not yet implemented)      │
├─────────────────────────────┤
│   Identity Mapped (4 GiB)   │ - For safe CR3 switch
└─────────────────────────────┘ 0x0
```

### Boot Process

1. **BIOS/UEFI** loads the Limine bootloader
2. **Limine** parses `limine.conf` and loads the kernel
3. **Kernel Entry** (`main.rs`) receives Limine protocol info:
   - Framebuffer
   - Memory map
   - SMP (multiprocessor) info
   - ACPI/Device Tree tables
4. **Initialization Sequence**:
   ```
   serial::init()     → Initialize console and logger
   memory::init()     → Set up frame allocator & page tables
   arch::init()       → Architecture-specific (GDT, IDT, CR3)
   allocator::init()  → Initialize heap allocator
   SMP bootstrap      → Start secondary CPUs
   ```

### Key Modules

| Module | Purpose |
|--------|---------|
| `memory` | Physical frame allocation, virtual memory mapping |
| `arch` | Architecture-specific code (GDT, IDT, page tables) |
| `allocator` | Global heap allocator using `slab_allocator_rs` |
| `serial` | UART driver for console output and logging |

## Development

### CI/CD Workflows

| Workflow | Trigger | Description |
|----------|---------|-------------|
| `lint.yml` | Push/PR | `cargo fmt` and `cargo clippy` checks |
| `audit.yml` | Push/PR + Daily | Security vulnerability scanning |
| `deny.yml` | Push/PR + Daily | License and dependency checks |

### Git Hooks

This project uses `husky-rs` for git hooks. To activate:

```bash
cargo build
```

**Pre-commit & Pre-push hooks run:**
- `cargo fmt --all -- --check` - Code formatting check
- `cargo clippy -- -D warnings` - Linting with warnings as errors

To skip hooks (not recommended):
```bash
NO_HUSKY_HOOKS=1 cargo build
```

### Code Style

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check formatting without modifying
cargo fmt -- --check
```

### Security Checks

```bash
# Check for known vulnerabilities
cargo audit

# Check licenses and dependencies
cargo deny check
```

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run checks:
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo audit
   cargo deny check
   ```
5. Commit your changes (hooks will run automatically)
6. Push to the branch
7. Open a Pull Request

## Roadmap

- [ ] User mode support
- [ ] System calls
- [ ] File system
- [ ] Network stack
- [ ] GUI/Window manager
- [ ] Device drivers (keyboard, mouse, disk)

## Acknowledgments

- [Limine Bootloader](https://github.com/limine-bootloader/limine) - Modern bootloader protocol
- [page_table_multiarch](https://crates.io/crates/page_table_multiarch) - Multi-architecture paging
- [Writing an OS in Rust](https://os.phil-opp.com/) - Excellent OS development tutorial

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.
