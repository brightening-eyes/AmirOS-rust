//! loongarch64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_entry::loongarch64::LA64PTE;
use page_table_multiarch::loongarch64::LA64PageTable;

pub type PageTable = LA64PageTable<AmirOSPagingHandler>;
pub type PageTableEntry = LA64PTE;
