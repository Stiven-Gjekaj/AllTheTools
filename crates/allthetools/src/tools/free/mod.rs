//! `att free`: show the memory and the swap of the host.
//!
//! Linux has `free`. macOS has `vm_stat`, which counts pages and leaves the
//! sums to the reader, and Windows shows memory in Task Manager only. This
//! tool gives the same table on each host.

pub mod parse;
