//! `att uptime`: show how long the host has run since its start, and its
//! load.
//!
//! macOS and Linux have `uptime`. Windows shows the time since the start in
//! Task Manager only, and `systeminfo` takes seconds and writes a date in
//! the language of the host. This tool gives the same report on each host.

pub mod parse;
