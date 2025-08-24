//! x86_64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_multiarch::x86_64::X64PageTable;

/// A type alias for the x86_64-specific page table, using our OS's handler.
pub type PageTable = X64PageTable<AmirOSPagingHandler>;
