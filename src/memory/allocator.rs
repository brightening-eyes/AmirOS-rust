// allocator based on free list
use free_list::{AllocError, FreeList, PageLayout, PageRange};
use limine::memory_map::{Entry, EntryType};

pub struct FrameAllocator {
    allocator: FreeList<16>,
    pub hhdm_offset: usize,
}

// ensure safety via mutex
unsafe impl Send for FrameAllocator {}

impl FrameAllocator {
    #[must_use] 
    pub const fn new(hhdm_offset: usize) -> Self {
        Self {
            allocator: FreeList::new(),
            hhdm_offset,
        }
    }

    pub fn init(&mut self, memmap: &[&Entry]) {
        memmap
            .iter()
            .filter(|region| region.entry_type == EntryType::USABLE)
            .map(|region| {
                let start = region.base as usize;
                let end = start + region.length as usize;
                (start..end).try_into()
            })
            .filter_map(Result::ok)
            .for_each(|region: PageRange| {
                unsafe {
                    self.allocator
                        .deallocate(region)
                        .expect("failed to add the memory region to the allocator.");
                };
            });
        log::info!("freelist memory allocator initialized.");
    }

    pub fn allocate(&mut self, size: usize) -> Result<PageRange, AllocError> {
        let layout = PageLayout::from_size_align(size, size).unwrap();
        self.allocator.allocate(layout)
    }

    pub unsafe fn deallocate(&mut self, addr: PageRange) {
        unsafe { self.allocator.deallocate(addr).ok() };
    }
}
