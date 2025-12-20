use meminterval::{Interval, IntervalTree};
use memory_addr::VirtAddr;
use page_table_multiarch::MappingFlags;

#[derive(Clone, Copy)]
pub struct VirtualMemoryArea {
    pub flags: MappingFlags,
}

pub struct VirtualAddressSpace {
    vmas: IntervalTree<VirtAddr, VirtualMemoryArea>,
}

impl Default for VirtualAddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualAddressSpace {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            vmas: IntervalTree::new(),
        }
    }

    #[must_use] 
    pub fn find_free_area(&self, size: usize) -> Option<VirtAddr> {
        // A simple approach: start searching from the beginning of userspace
        let mut search_start_addr = VirtAddr::from_usize(0x1000); // Start at a safe, non-null address
        let mut search_end_addr = VirtAddr::from_usize(search_start_addr.as_usize() + size); // starting address + size
        loop {
            // Find the next interval that might overlap with our search address
            let next_entry = self
                .vmas
                .query(Interval::new(search_start_addr, search_end_addr))
                .find(|i| i.interval.end > search_start_addr);
            match next_entry {
                Some(entry) => {
                    // There's an interval entry after our search address.
                    // Check if there's enough space between the current search address
                    // and the start of the next interval.
                    if entry.interval.start > search_start_addr
                        && (entry.interval.start - search_start_addr) >= size
                    {
                        // We found a gap!
                        return Some(search_start_addr);
                    }
                    // No gap, so move our search address to the end of this interval
                    search_start_addr = entry.interval.end;
                    search_end_addr =
                        VirtAddr::from_usize(entry.interval.end.as_usize() + size);
                }
                None => {
                    // No more intervals, so this address is free
                    return Some(search_start_addr);
                }
            }
        }
    }

    pub fn allocate(&mut self, address: VirtAddr, size: usize, value: VirtualMemoryArea) {
        let end_address = VirtAddr::from_usize(address.as_usize() + size); // starting address + size
        self.vmas.insert(Interval::new(address, end_address), value);
    }

    pub fn dealloc(&mut self, address: VirtAddr, size: usize) {
        let end_address = VirtAddr::from_usize(address.as_usize() + size); // starting address + size
        self.vmas.delete(Interval::new(address, end_address));
    }
}
