#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use limine::*;
use limine::BaseRevision;
use limine::request::*;

// boot loader revision
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

// boot loader information
#[used]
#[unsafe(link_section = ".requests")]
static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();

// firmware type
#[used]
#[unsafe(link_section = ".requests")]
static FIRMWARE_TYPE_REQUEST: FirmwareTypeRequest = FirmwareTypeRequest::new();

// set stack size to 128 kb (will be back to it later)
#[used]
#[unsafe(link_section = ".requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(128 * 1024);

// hier half direct mapping
#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

// frame buffer
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

// paging mode
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[used]
#[unsafe(link_section = ".requests")]
static PAGING_MODE_REQUEST: PagingModeRequest = PagingModeRequest::new().with_mode(paging::Mode::FOUR_LEVEL);

#[cfg(target_arch = "riscv64")]
#[used]
#[unsafe(link_section = ".requests")]
static PAGING_MODE_REQUEST: PagingModeRequest = PagingModeRequest::new().with_mode(paging::Mode::SV48);

// bootstrap all cores on the system
#[used]
#[unsafe(link_section = ".requests")]
static MP_REQUEST: MpRequest = MpRequest::new();

// memory maps
#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

// kernel information
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();

// rsdp
#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

// smbios information
#[used]
#[unsafe(link_section = ".requests")]
static SMBIOS_REQUEST: SmbiosRequest = SmbiosRequest::new();

// uefi table
#[used]
#[unsafe(link_section = ".requests")]
static EFI_SYSTEM_TABLE_REQUEST: EfiSystemTableRequest = EfiSystemTableRequest::new();

// uefi memory map
#[used]
#[unsafe(link_section = ".requests")]
static EFI_MEMORY_MAP_REQUEST: EfiMemoryMapRequest = EfiMemoryMapRequest::new();

// boot time
#[used]
#[unsafe(link_section = ".requests")]
static DATE_AT_BOOT_REQUEST: DateAtBootRequest = DateAtBootRequest::new();

// kernel address
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

// device tree blob
#[used]
#[unsafe(link_section = ".requests")]
static DEVICE_TREE_BLOB_REQUEST: DeviceTreeBlobRequest = DeviceTreeBlobRequest::new();

// bsp Hart ID for riscv64
#[cfg(target_arch = "riscv64")]
#[used]
#[unsafe(link_section = ".requests")]
static BSP_HARTID_REQUEST: BspHartidRequest = BspHartidRequest::new();

// start and end markers for limine
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
pub extern "C" fn main() -> !{
if !BASE_REVISION.is_supported()
{
holt();
}
if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response()
{
if let Some(framebuffer) = framebuffer_response.framebuffers().next()
{
for i in 0..100_u64
{
// Calculate the pixel offset using the framebuffer information we obtained above.
// We skip `i` scanlines (pitch is provided in bytes) and add `i * 4` to skip `i` pixels forward.
let pixel_offset = i * framebuffer.pitch() + i * 4;
// Write 0xFFFFFFFF to the provided pixel offset to fill it white.
unsafe
{
framebuffer.addr().add(pixel_offset as usize).cast::<u32>().write(0xFFFFFFFF)
                };
            }
        }
}
loop
{

}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
holt();
}

fn holt() -> ! {
    loop {
        unsafe {
            #[cfg(target_arch = "x86_64")]
            asm!("hlt");
            #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
            asm!("wfi");
            #[cfg(target_arch = "loongarch64")]
            asm!("idle 0");
        }
    }
}
