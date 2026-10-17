//! `att flushdns`: empty the DNS cache of the host, so the next lookup of
//! each name asks the DNS server again.
//!
//! Each host has its own command for this, and macOS needs two of them:
//! `ipconfig /flushdns` on Windows, `dscacheutil` and a signal to
//! mDNSResponder on macOS, and `resolvectl` or `nscd` on Linux. This tool is
//! one command on each host.

pub mod plan;
