//! riscv64-specific page table type definition.

use crate::memory::paging::AmirOSPagingHandler;
use page_table_multiarch::riscv64::Riscv64PageTable;

/// A type alias for the riscv64-specific page table, using our OS's handler.
/// This is the only thing this module exports.
pub type PageTable = Riscv64PageTable<AmirOSPagingHandler>;
