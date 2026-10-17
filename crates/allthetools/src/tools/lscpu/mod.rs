//! `att lscpu`: show the processor: its model, its cores and threads, its
//! caches and its features.
//!
//! Linux has `lscpu`. macOS keeps these values in sysctl, under names that a
//! person must know, and Windows shows them in Task Manager only. This tool
//! gives the same report on each host.

pub mod parse;
