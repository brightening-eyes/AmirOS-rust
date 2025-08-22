//interrupt descriptor table (IDT)
use super::gdt;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;

lazy_static!
{
static ref IDT: InterruptDescriptorTable = {
let mut idt = InterruptDescriptorTable::new();
idt.breakpoint.set_handler_fn(breakpoint_handler);
idt.page_fault.set_handler_fn(page_fault_handler);
unsafe
{
idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
}

idt
    };
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame)
{
log::info!("EXCEPTION: BREAKPOINT");
}

extern "x86-interrupt" fn page_fault_handler(_frame: InterruptStackFrame, _error_code: PageFaultErrorCode)
{
panic!("page fucking fault!.");
}

extern "x86-interrupt" fn double_fault_handler(_stack: InterruptStackFrame, _error_code: u64) -> !
{
panic!("double fault!.");
}

pub fn init()
{
IDT.load();
}
