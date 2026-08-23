//! AmirOS hardware abstraction layer.
//!
//! Defines arch-neutral, object-safe traits plus global registries that the
//! active architecture populates exactly once during `amir_arch::init()`.
//! Consumers access implementations through [`irq`], [`timer`], [`percpu`],
//! [`uart`], [`framebuffer`] and construct address spaces via
//! [`AddressSpaceRegistry::create_address_space`].
//!
//! Two dispatch paths exist by design (see the workspace plan):
//! - Registry objects (`&'static dyn`) for devices/controllers — everything
//!   in this file.
//! - A direct `cfg`-selected path inside `amir_arch` for per-CPU primitives
//!   that cannot be expressed as object-safe calls (context switching,
//!   interrupt flag manipulation).

#![no_std]

pub mod bottom_half;

use core::fmt;
use memory_addr::{PhysAddr, VirtAddr};
use page_table_multiarch::{MappingFlags, PageSize};

/// Identifies an interrupt line within the active controller's namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrqNumber(pub u16);

/// Identifies a CPU in the system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuId(pub usize);

/// State captured when an interrupt or exception is delivered.
///
/// Populated by the architecture layer; contents grow in later phases.
#[derive(Debug)]
pub struct IrqContext {
    /// Faulting/originating virtual address where meaningful (e.g., CR2).
    pub fault_addr: Option<VirtAddr>,
}

impl fmt::Display for IrqContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.fault_addr {
            Some(addr) => write!(f, "fault_addr={addr:#x}"),
            None => write!(f, "no fault address"),
        }
    }
}

/// Interrupt controller services.
pub trait IrqController: Sync + Send {
    /// Enables interrupt delivery on the calling CPU.
    fn enable(&self);
    /// Disables interrupt delivery on the calling CPU.
    fn disable(&self);
    /// Registers `handler` for `irq`. Errors if already registered.
    ///
    /// # Errors
    /// Returns the handler back unchanged when the line is taken.
    fn register(
        &self,
        irq: IrqNumber,
        handler: fn(&mut IrqContext),
    ) -> Result<(), fn(&mut IrqContext)>;
    /// Signals end-of-interrupt for a level-triggered line.
    fn eoi(&self, irq: IrqNumber);
}

/// Timer services.
pub trait Timer: Sync + Send {
    /// Monotonic nanoseconds since some arbitrary epoch.
    fn now_ns(&self) -> u64;
    /// Arms a one-shot expiry at absolute monotonic `deadline_ns`.
    fn set_oneshot(&self, deadline_ns: u64);
}

/// Errors produced by address-space operations.
#[derive(Clone, Copy, Debug)]
pub enum MapError {
    /// The mapping already exists at this address.
    AlreadyMapped,
    /// Out of physical frames or page-table pages.
    OutOfMemory,
    /// The operation is not supported by this implementation yet.
    Unsupported,
}

/// A single address space (page-table root plus its mappings).
pub trait AddressSpace: Sync + Send {
    /// Maps one page of `size` at `vaddr` backed by `paddr`.
    ///
    /// # Errors
    /// See [`MapError`].
    fn map(
        &self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: PageSize,
        flags: MappingFlags,
    ) -> Result<(), MapError>;
    /// Removes the mapping at `vaddr`, returning its physical address.
    ///
    /// # Errors
    /// Returns [`MapError::AlreadyMapped`]-style inversion via
    /// [`MapError::Unsupported`] until precise errors land in Phase 4.
    fn unmap(&self, vaddr: VirtAddr) -> Result<PhysAddr, MapError>;
    /// Translates `vaddr` to its physical address, if mapped.
    fn translate(&self, vaddr: VirtAddr) -> Option<PhysAddr>;
    /// Loads this address space into the calling CPU.
    fn switch_to(&self);
}

/// Per-CPU identity services.
pub trait PerCpu: Sync + Send {
    /// Returns the identifier of the calling CPU.
    fn cpu_id(&self) -> CpuId;
}

/// Byte-oriented UART console.
pub trait Uart: Sync + Send {
    /// Writes one byte, blocking until accepted by hardware.
    fn write_byte(&self, b: u8);
    /// Polls for one received byte, if any.
    fn read_byte(&self) -> Option<u8>;
}

/// A rectangle in framebuffer coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Framebuffer metadata handed over by the bootloader.
#[derive(Clone, Copy, Debug)]
pub struct FbInfo {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Bits per pixel.
    pub bpp: u32,
    /// Bytes per scanline.
    pub stride: usize,
    /// Mapped base address of the framebuffer.
    pub base: *mut u8,
}

/// Display output services over the boot framebuffer.
pub trait Framebuffer: Sync + Send {
    /// Static description of the display.
    fn info(&self) -> FbInfo;
    /// Copies `pixels` into `rect`; layout matches current pixel format.
    fn blit(&self, rect: Rect, pixels: &[u8]);
}

macro_rules! hal_registry {
    ($mod_name:ident, $trait_:ident, $doc:expr) => {
        #[doc = $doc]
        pub mod $mod_name {
            use super::$trait_;
            use spin::RwLock;

            static REGISTRY: RwLock<Option<&'static dyn $trait_>> = RwLock::new(None);

            /// Registers the implementation. Must be called once, during
            /// architecture initialization, before any consumer runs.
            ///
            /// # Panics
            /// Panics if an implementation was already registered.
            pub fn register(imp: &'static dyn $trait_) {
                let mut slot = REGISTRY.write();
                assert!(
                    slot.is_none(),
                    concat!("hal: ", stringify!($trait_), " registered twice")
                );
                *slot = Some(imp);
            }

            /// Returns the registered implementation.
            ///
            /// # Panics
            /// Panics if called before [`register`].
            pub fn get() -> &'static dyn $trait_ {
                slot().expect(concat!(
                    "hal: ",
                    stringify!($trait_),
                    " used before registration"
                ))
            }

            fn slot() -> Option<&'static dyn $trait_> {
                *REGISTRY.read()
            }
        }
    };
}

hal_registry!(
    irq,
    IrqController,
    "Access to the registered [`IrqController`]."
);
hal_registry!(timer, Timer, "Access to the registered [`Timer`].");
hal_registry!(percpu, PerCpu, "Access to the registered [`PerCpu`].");
hal_registry!(uart, Uart, "Access to the registered [`Uart`].");
hal_registry!(
    framebuffer,
    Framebuffer,
    "Access to the registered [`Framebuffer`]."
);

/// Address spaces cannot live in the macro registry above because they are
/// created dynamically rather than registered once; the active architecture
/// installs a factory instead.
pub mod address_space {
    use super::{AddressSpace, MapError};
    use memory_addr::{PhysAddr, VirtAddr};
    use spin::RwLock;

    /// Factory creating empty address spaces for the running architecture.
    pub trait AddressSpaceFactory: Sync + Send {
        /// Creates an empty address space (kernel-only mappings absent).
        ///
        /// # Errors
        /// Returns [`MapError::OutOfMemory`] when page-table pages cannot be
        /// allocated.
        fn create(&self) -> Result<&'static dyn AddressSpace, MapError>;
    }

    static FACTORY: RwLock<Option<&'static dyn AddressSpaceFactory>> = RwLock::new(None);

    /// Registers the architecture's address-space factory.
    ///
    /// # Panics
    /// Panics if a factory was already registered.
    pub fn register_factory(factory: &'static dyn AddressSpaceFactory) {
        let mut slot = FACTORY.write();
        assert!(slot.is_none(), "hal: AddressSpaceFactory registered twice");
        *slot = Some(factory);
    }

    /// Creates a fresh address space through the registered factory.
    ///
    /// # Panics
    /// Panics if called before [`register_factory`].
    ///
    /// # Errors
    /// Propagates the factory error.
    pub fn create() -> Result<&'static dyn AddressSpace, MapError> {
        (*FACTORY.read())
            .expect("hal: address-space factory used before registration")
            .create()
    }

    /// Convenience wrapper translating through an existing address space.
    pub fn translate_in(space: &dyn AddressSpace, vaddr: VirtAddr) -> Option<PhysAddr> {
        space.translate(vaddr)
    }
}
