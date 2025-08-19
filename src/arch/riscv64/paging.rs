//! riscv64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use lazy_static::lazy_static;
use page_table_multiarch::riscv64::Riscv64PageTable;
use spin::Mutex;

/// A type alias for the riscv64-specific page table, using our OS's handler.
pub type PageTable = Riscv64PageTable<AmirOSPagingHandler>;

lazy_static!
{
pub static ref PAGE_MAPPER: Mutex<PageTable> = {
let page_table = PageTable::try_new().expect("Failed to create riscv64 page table");
Mutex::new(page_table)
    };
}

/// Initializes and activates the riscv64 page table.
///
/// This function is called from the main architecture initialization routine.
pub fn init()
{    
let _root_paddr = PAGE_MAPPER.lock().root_paddr();
    log::info!("riscv64 paging initialized and activated.");
}
