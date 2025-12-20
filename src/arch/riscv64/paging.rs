//! riscv64-specific paging implementation and initialization.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_entry::riscv::Rv64PTE;
use page_table_multiarch::riscv::Sv48PageTable;

/// A type alias for the riscv64-specific page table, using our OS's handler.
pub type PageTable = Sv48PageTable<AmirOSPagingHandler>;
pub type PageTableEntry = Rv64PTE;
