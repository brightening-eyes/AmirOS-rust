use memory_addr::VirtAddr;
use meminterval::{IntervalTree, Interval};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Permissions
{
pub read: bool,
pub write: bool,
pub execute: bool,
}

pub struct VirtualMemoryArea
{
interval: Interval<VirtAddr>,
permissions: Permissions,
}

pub struct VirtualAddressSpace
{
vmas: IntervalTree<VirtAddr, VirtualMemoryArea>,
}

impl Default for VirtualAddressSpace
{
    fn default() -> Self
{
        Self::new()
    }
}

impl VirtualAddressSpace
{

pub fn new() -> Self
{
Self { vmas: IntervalTree::new(), }
}

    pub fn find_free_area(&self, size: usize) -> Option<VirtAddr>
{
// A simple approach: start searching from the beginning of userspace
let mut search_start_addr = VirtAddr::from_usize(0x1000); // Start at a safe, non-null address
let mut search_end_addr = VirtAddr::from_usize(search_start_addr.as_usize() + size); // starting address + size


loop
{
// Find the next interval that might overlap with our search address
let next_entry = self.vmas.query(Interval::new(search_start_addr, search_end_addr)).find(|i| i.interval.end > search_start_addr);
match next_entry
{
Some(entry) =>
{
// There's an interval entry after our search address.
// Check if there's enough space between the current search address
// and the start of the next interval.
if entry.interval.start > search_start_addr && (entry.interval.start - search_start_addr) >= size
{
// We found a gap!
return Some(search_start_addr);
}
else
{
// No gap, so move our search address to the end of this interval
search_start_addr = entry.interval.end;
search_end_addr = VirtAddr::from_usize(entry.interval.end.as_usize() + size);
}
}
None =>
{
// No more intervals, so this address is free
return Some(search_start_addr);
}
}
}
}

}
