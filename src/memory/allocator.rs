// allocator based on free list
use free_list::{AllocError, FreeList, PageLayout, PageRange};
use limine::memmap::{Entry, MEMMAP_USABLE};

pub struct FrameAllocator {
    allocator: FreeList<16>,
    pub hhdm_offset: usize,
}

// Safety: FrameAllocator wraps FreeList<16> and a usize. FreeList contains
// a SmallVec of PageRange (all Copy types), making it automatically Send+Sync.
// External synchronization via RwLock prevents concurrent access.
unsafe impl Send for FrameAllocator {}
unsafe impl Sync for FrameAllocator {}

impl FrameAllocator {
    #[must_use]
    pub const fn new(hhdm_offset: usize) -> Self {
        Self {
            allocator: FreeList::new(),
            hhdm_offset,
        }
    }

    /// initialization code for `frame allocator`.
    /// initializes the free memory based on the provided memory information from the boot loader
    /// # Panics
    /// when the free list allocator cant grab the memory
    pub fn init(&mut self, memmap: &[&Entry]) {
        memmap
            .iter()
            .filter(|region| region.type_ == MEMMAP_USABLE)
            .map(|region| {
                let start =
                    usize::try_from(region.base).expect("allocator: invalid base in memory region");
                let length = usize::try_from(region.length)
                    .expect("allocator: invalid length in memory region");
                let end = start
                    .checked_add(length)
                    .expect("allocator: integer overflow in memory region calculation");
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

    /// allocates and returns memory based on the available free memory
    /// # Errors
    /// when no memory is available on the free list to allocate, we will get an allocation error.
    pub fn allocate(&mut self, layout: PageLayout) -> Result<PageRange, AllocError> {
        self.allocator.allocate(layout)
    }

    pub fn deallocate(&mut self, addr: PageRange) {
        unsafe { self.allocator.deallocate(addr).ok() };
    }
}
