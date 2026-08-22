//! ACPI discovery: RSDP → MADT, producing the platform's interrupt topology
//! (LAPIC base, IO-APICs, legacy-PIC requirement).
//!
//! Runs after the kernel heap is live because the `acpi` crate allocates.
//! AML interpretation is disabled (no `aml` feature) — we only read static
//! tables, so the handler's timing hooks are coarse TSC approximations.

use acpi::{
    AcpiTables,
    sdt::madt::{Madt, MadtEntry},
};
use core::pin::Pin;
use core::ptr::NonNull;
use pci_types::PciAddress;
use spin::Mutex;

/// One discovered IO-APIC.
#[derive(Clone, Copy, Debug)]
pub struct IoApic {
    pub paddr: u64,
    pub gsi_base: u32,
}

/// The MADT-derived subset of the platform we care about in P1.
#[derive(Clone, Debug)]
pub struct ApicPlatform {
    /// Physical address of the (BSP) local APIC.
    pub lapic_paddr: usize,
    pub io_apics: [Option<IoApic>; MAX_IO_APICS],
    pub io_apic_count: usize,
    /// ACPI reports legacy 8259s alongside the APIC.
    pub has_legacy_pics: bool,
}

const MAX_IO_APICS: usize = 4;

static PLATFORM: Mutex<Option<ApicPlatform>> = Mutex::new(None);

/// Returns the discovered platform, if [`discover`] already ran.
pub fn platform() -> Option<ApicPlatform> {
    PLATFORM.lock().clone()
}

/// `acpi` crate accessors routed through the higher-half direct map.
/// HHDM covers all physical memory, so mapping is pure arithmetic; the
/// timing hooks are only used by AML execution, which never happens here.
#[derive(Clone, Copy)]
struct HhdmHandler;

impl HhdmHandler {
    fn virt(&self, address: usize) -> *mut u8 {
        let hhdm = amir_mm::FRAME_ALLOCATOR.read().hhdm_offset;
        (address + hhdm) as *mut u8
    }
}

impl acpi::Handler for HhdmHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let aligned = physical_address & !0xFFF;
        let offset = physical_address - aligned;
        let virtual_start = unsafe {
            // Safety: HHDM guarantees the region is mapped and readable.
            NonNull::new_unchecked(self.virt(aligned).byte_add(offset).cast::<T>())
        };
        acpi::PhysicalMapping {
            physical_start: physical_address,
            virtual_start,
            region_length: size,
            mapped_length: (offset + size + 0xFFF) & !0xFFF,
            handler: *self,
        }
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {}

    fn read_u8(&self, address: usize) -> u8 {
        unsafe { self.virt(address).read_volatile() }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe { self.virt(address).cast::<u16>().read_volatile() }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe { self.virt(address).cast::<u32>().read_volatile() }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe { self.virt(address).cast::<u64>().read_volatile() }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe { self.virt(address).write_volatile(value) }
    }
    fn write_u16(&self, address: usize, value: u16) {
        unsafe { self.virt(address).cast::<u16>().write_volatile(value) }
    }
    fn write_u32(&self, address: usize, value: u32) {
        unsafe { self.virt(address).cast::<u32>().write_volatile(value) }
    }
    fn write_u64(&self, address: usize, value: u64) {
        unsafe { self.virt(address).cast::<u64>().write_volatile(value) }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        crate::irq::port_read_u8(port)
    }
    fn read_io_u16(&self, port: u16) -> u16 {
        crate::irq::port_read_u16(port)
    }
    fn read_io_u32(&self, port: u16) -> u32 {
        crate::irq::port_read_u32(port)
    }
    fn write_io_u8(&self, port: u16, value: u8) {
        crate::irq::port_write_u8(port, value);
    }
    fn write_io_u16(&self, port: u16, value: u16) {
        crate::irq::port_write_u16(port, value);
    }
    fn write_io_u32(&self, port: u16, value: u32) {
        crate::irq::port_write_u32(port, value);
    }

    fn read_pci_u8(&self, address: PciAddress, offset: u16) -> u8 {
        pci_cam_read(address, offset, |v| v as u8)
    }
    fn read_pci_u16(&self, address: PciAddress, offset: u16) -> u16 {
        pci_cam_read(address, offset, |v| v as u16)
    }
    fn read_pci_u32(&self, address: PciAddress, offset: u16) -> u32 {
        pci_cam_read(address, offset, |v| v)
    }
    fn write_pci_u8(&self, address: PciAddress, offset: u16, value: u8) {
        pci_cam_write(address, offset, u32::from(value));
    }
    fn write_pci_u16(&self, address: PciAddress, offset: u16, value: u16) {
        pci_cam_write(address, offset, u32::from(value));
    }
    fn write_pci_u32(&self, address: PciAddress, offset: u16, value: u32) {
        pci_cam_write(address, offset, value);
    }

    // Monotonic nanoseconds. Only consumed by AML sleep paths, which this
    // kernel never runs; a fixed ~1 GHz TSC assumption keeps it monotonic.
    fn nanos_since_boot(&self) -> u64 {
        crate::pit::read_tsc()
    }
    fn stall(&self, microseconds: u64) {
        let end = crate::pit::read_tsc().wrapping_add(microseconds.saturating_mul(1000));
        while crate::pit::read_tsc() < end {
            core::hint::spin_loop();
        }
    }
    fn sleep(&self, milliseconds: u64) {
        self.stall(milliseconds.saturating_mul(1000));
    }

    // AML mutex hooks. This kernel never interprets AML, so handles are
    // minted from a counter and locking is vacuous (single-tasked boot).
    fn create_mutex(&self) -> acpi::Handle {
        static NEXT_ID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
        acpi::Handle(NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }
    fn acquire(&self, _mutex: acpi::Handle, _timeout: u16) -> Result<(), acpi::aml::AmlError> {
        Ok(())
    }
    fn release(&self, _mutex: acpi::Handle) {}
}

/// Reads through PCI configuration space via CAM (ports 0xCF8/0xCFC).
fn pci_cam_read<R>(address: PciAddress, offset: u16, narrow: fn(u32) -> R) -> R {
    assert_eq!(
        address.segment(),
        0,
        "acpi: CAM cannot reach non-zero PCI segments"
    );
    crate::irq::port_write_u32(0xCF8, cam_address(address, offset));
    narrow(crate::irq::port_read_u32(0xCFC))
}

fn pci_cam_write(address: PciAddress, offset: u16, value: u32) {
    assert_eq!(
        address.segment(),
        0,
        "acpi: CAM cannot reach non-zero PCI segments"
    );
    crate::irq::port_write_u32(0xCF8, cam_address(address, offset));
    crate::irq::port_write_u32(0xCFC, value);
}

fn cam_address(address: PciAddress, offset: u16) -> u32 {
    1 << 31
        | (u32::from(address.bus()) << 16)
        | (u32::from(address.device()) << 11)
        | (u32::from(address.function()) << 8)
        | u32::from(offset & 0xFFFC)
}

/// Parses the RSDP at `rsdp_paddr` and extracts the MADT topology.
///
/// # Panics
/// Panics when ACPI is present but unusable — without the MADT the platform
/// cannot bring up external interrupts safely.
pub fn discover(rsdp_paddr: Option<usize>) {
    let Some(rsdp_paddr) = rsdp_paddr else {
        log::warn!("acpi: no RSDP from bootloader; running PIC-only");
        return;
    };

    // Safety: the bootloader hands us a valid RSDP; all reads are read-only.
    let tables = unsafe { AcpiTables::<HhdmHandler>::from_rsdp(HhdmHandler, rsdp_paddr) }
        .expect("acpi: failed to parse system tables");

    let Some(madt_mapping) = tables.find_table::<Madt>() else {
        log::warn!("acpi: no MADT found; running PIC-only");
        return;
    };

    // Safety: the mapping stays valid (HHDM-backed) for the loop below, and
    // MADT is declared `PhantomPinned`, so pinning is sound.
    let madt: Pin<&Madt> = unsafe { Pin::new_unchecked(madt_mapping.virtual_start.as_ref()) };

    let mut lapic_base = u64::from(madt.local_apic_address);
    let mut io_apics = [const { None }; MAX_IO_APICS];
    let mut io_apic_count = 0usize;
    for entry in madt.entries() {
        match entry {
            MadtEntry::LocalApicAddressOverride(e) => lapic_base = e.local_apic_address,
            MadtEntry::IoApic(e) => {
                assert!(io_apic_count < MAX_IO_APICS, "acpi: too many IO-APICs");
                io_apics[io_apic_count] = Some(IoApic {
                    paddr: u64::from(e.io_apic_address),
                    gsi_base: e.global_system_interrupt_base,
                });
                io_apic_count += 1;
            }
            _ => {}
        }
    }

    let has_legacy_pics = madt.supports_8259();
    let lapic_paddr = usize::try_from(lapic_base).expect("acpi: LAPIC address overflow");
    log::info!(
        "acpi: LAPIC @ {:#x}, {} IO-APIC(s), legacy PICs: {}",
        lapic_paddr,
        io_apic_count,
        has_legacy_pics
    );
    *PLATFORM.lock() = Some(ApicPlatform {
        lapic_paddr,
        io_apics,
        io_apic_count,
        has_legacy_pics,
    });
}
