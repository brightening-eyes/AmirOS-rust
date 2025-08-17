//! x86_64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use lazy_static::lazy_static;
use page_table_multiarch::x86_64::X64PageTable;
use spin::Mutex;

/// A type alias for the x86_64-specific page table, using our OS's handler.
pub type PageTable = X64PageTable<AmirOSPagingHandler>;

lazy_static!
{
pub static ref PAGE_MAPPER: Mutex<PageTable> = {
let page_table = PageTable::try_new().expect("Failed to create x86_64 page table");
Mutex::new(page_table)
    };
}

/// Initializes and activates the x86_64 page table.
///
/// This function is called from the main architecture initialization routine.
pub fn init()
{    
let _root_paddr = PAGE_MAPPER.lock().root_paddr();
    log::info!("x86_64 paging initialized and activated.");
}
