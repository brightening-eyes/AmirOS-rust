//! x86_64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_multiarch::x86_64::X64PageTable;
use page_table_entry::x86_64::X64PTE;

/// type aliases for x86_64 paging
pub type PageTable = X64PageTable<AmirOSPagingHandler>;
pub type PageTableEntry = X64PTE;
