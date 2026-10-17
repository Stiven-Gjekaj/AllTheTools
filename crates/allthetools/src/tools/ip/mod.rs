//! `att ip`: list the network interfaces with their state, their hardware
//! address and their IP addresses.
//!
//! Each host has a command for this, and each one has its own output: `ip`
//! on Linux, `ifconfig` on macOS and `ipconfig` on Windows. This tool gives
//! one table on each host, as `ip -brief address` does.

pub mod parse;
