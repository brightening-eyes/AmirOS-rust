//! aarch64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_entry::aarch64::A64PTE;
use page_table_multiarch::aarch64::A64PageTable;

pub type PageTable = A64PageTable<AmirOSPagingHandler>;
pub type PageTableEntry = A64PTE;
