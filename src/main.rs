#![no_std]
#![no_main]
// architecture-specific compiler features
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

//module declarations
pub mod allocator;
pub mod arch;
pub mod heap;
pub mod memory;
pub mod serial;

//crate imports and usages
use core::panic::PanicInfo;
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "riscv64",
    target_arch = "aarch64"
))]
use limine::paging::Mode;
#[cfg(target_arch = "riscv64")]
use limine::request::BspHartidRequest;
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "riscv64",
    target_arch = "aarch64"
))]
use limine::request::PagingModeRequest;
use limine::request::{
    BootloaderInfoRequest, DateAtBootRequest, DeviceTreeBlobRequest, EfiMemoryMapRequest,
    EfiSystemTableRequest, ExecutableAddressRequest, ExecutableFileRequest, FirmwareTypeRequest,
    FramebufferRequest, HhdmRequest, MemoryMapRequest, MpRequest, RequestsEndMarker,
    RequestsStartMarker, RsdpRequest, SmbiosRequest, StackSizeRequest,
};
use limine::BaseRevision;

// boot loader revision
#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

// boot loader information
#[used]
#[unsafe(link_section = ".limine_requests")]
static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();

// firmware type
#[used]
#[unsafe(link_section = ".limine_requests")]
static FIRMWARE_TYPE_REQUEST: FirmwareTypeRequest = FirmwareTypeRequest::new();

// set stack size to 128 kb (will be back to it later)
#[used]
#[unsafe(link_section = ".limine_requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(128 * 1024);

// hier half direct mapping
#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

// frame buffer
#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

// paging mode
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[used]
#[unsafe(link_section = ".limine_requests")]
static PAGING_MODE_REQUEST: PagingModeRequest =
    PagingModeRequest::new().with_mode(Mode::FOUR_LEVEL);

#[cfg(target_arch = "riscv64")]
#[used]
#[unsafe(link_section = ".limine_requests")]
static PAGING_MODE_REQUEST: PagingModeRequest = PagingModeRequest::new().with_mode(Mode::SV48);

// bootstrap all cores on the system
#[used]
#[unsafe(link_section = ".limine_requests")]
static MP_REQUEST: MpRequest = MpRequest::new();

// memory maps
#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

// kernel information
#[used]
#[unsafe(link_section = ".limine_requests")]
static EXECUTABLE_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();

// rsdp
#[used]
#[unsafe(link_section = ".limine_requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

// smbios information
#[used]
#[unsafe(link_section = ".limine_requests")]
static SMBIOS_REQUEST: SmbiosRequest = SmbiosRequest::new();

// uefi table
#[used]
#[unsafe(link_section = ".limine_requests")]
static EFI_SYSTEM_TABLE_REQUEST: EfiSystemTableRequest = EfiSystemTableRequest::new();

// uefi memory map
#[used]
#[unsafe(link_section = ".limine_requests")]
static EFI_MEMORY_MAP_REQUEST: EfiMemoryMapRequest = EfiMemoryMapRequest::new();

// boot time
#[used]
#[unsafe(link_section = ".limine_requests")]
static DATE_AT_BOOT_REQUEST: DateAtBootRequest = DateAtBootRequest::new();

// kernel address
#[used]
#[unsafe(link_section = ".limine_requests")]
static EXECUTABLE_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

// device tree blob
#[used]
#[unsafe(link_section = ".limine_requests")]
static DEVICE_TREE_BLOB_REQUEST: DeviceTreeBlobRequest = DeviceTreeBlobRequest::new();

// bsp Hart ID for riscv64
#[cfg(target_arch = "riscv64")]
#[used]
#[unsafe(link_section = ".limine_requests")]
static BSP_HARTID_REQUEST: BspHartidRequest = BspHartidRequest::new();

// start and end markers for limine
#[used]
#[unsafe(link_section = ".limine_requests_start")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".limine_requests_end")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    serial::init();
    log::info!("logger initialized");
    assert!(
        BASE_REVISION.is_supported(),
        "boot loader base revision not supported!."
    );
    log::info!("base revision supported");
    if let Some(info) = BOOTLOADER_INFO_REQUEST.get_response() {
        log::info!("Booted by: {} v{}", info.name(), info.version());
    } else {
        panic!("boot loader information not available.");
    }
    if let Some(_framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        log::info!("we have the frame buffer, we'll do for it later");
    }
    memory::init(MEMORY_MAP_REQUEST.get_response().unwrap().entries());
    log::info!("memory manager initialized.");
    arch::init();
    log::info!("architecture initialization complete.");
    allocator::init();
    log::info!("allocator initialized.");
    if let Some(mp_response) = MP_REQUEST.get_response() {
        // Get the BSP's unique ID in an architecture-agnostic way.
        /*let bsp_id =
        {
        #[cfg(target_arch = "x86_64")]
        { mp_response.bsp_lapic_id() }
        #[cfg(target_arch = "riscv64")]
        { crate::BSP_HARTID_REQUEST.get_response().unwrap().id() }
        #[cfg(target_arch = "aarch64")]
        { mp_response.bsp_mpidr() }
        };*/
        log::info!("SMP support detected.");
        #[allow(unused_variables)]
        for cpu in mp_response.cpus() {
            /*let cpu_id =
            {
            #[cfg(target_arch = "x86_64")]
            { cpu.lapic_id }
            #[cfg(target_arch = "riscv64")]
            { cpu.hartid }
            #[cfg(target_arch = "aarch64")]
            { cpu.mpidr }
            };*/
            /*if cpu_id == bsp_id
            {
            continue;
            }*/
            #[cfg(not(target_arch = "loongarch64"))]
            cpu.goto_address.write(os_loop);
            #[cfg(target_arch = "loongarch64")]
            log::warn!("SMP not yet supported on loongarch64");
        }
    }
    loop {
        arch::holt();
    }
}

pub extern "C" fn os_loop(_cpu: &limine::mp::Cpu) -> ! {
    log::info!("processor started.");
    loop {
        arch::holt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{info}");
    loop {
        arch::holt();
    }
}
