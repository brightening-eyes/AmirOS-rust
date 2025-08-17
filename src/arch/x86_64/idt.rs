//interrupt descriptor table (IDT)
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;

lazy_static!
{
static ref IDT: InterruptDescriptorTable = {
let mut idt = InterruptDescriptorTable::new();
idt.breakpoint.set_handler_fn(breakpoint_handler);
idt.page_fault.set_handler_fn(page_fault_handler);

idt
    };
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame)
{
log::info!("EXCEPTION: BREAKPOINT");
}

extern "x86-interrupt" fn page_fault_handler(_frame: InterruptStackFrame, _error_code: PageFaultErrorCode)
{
log::info!("page fucking fault!.");
}

pub fn init()
{
    IDT.load();
}
