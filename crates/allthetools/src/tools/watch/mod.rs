//! `att watch`: run a command again and again, and show its output on a
//! clear screen.
//!
//! Linux has `watch` in procps. macOS and Windows have no such command. This
//! tool takes the main options of `watch`: the interval, the marks on the
//! changes, and an exit when the output changes or the command fails.

pub mod screen;
