use memory_addr::VirtAddr;
use meminterval::{IntervalTree, Interval};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Permissions
{
pub read: bool,
pub write: bool,
pub execute: bool,
}

#[derive(Clone)]
pub struct VirtualMemoryArea
{
interval: Interval<VirtAddr>,
permissions: Permissions,
}

pub struct VirtualAddressSpace
{
vmas: IntervalTree<VirtAddr, VirtualMemoryArea>,
}

impl VirtualAddressSpace
{

pub fn new() -> Self
{
Self { vmas: IntervalTree::new(), }
}

}
