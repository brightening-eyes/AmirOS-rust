#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};

// boot loader revision
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

// frame buffer
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

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
