//! `att say`: read text aloud with a voice of the host.
//!
//! macOS has `say`. Windows has voices and no command that uses them, and a
//! Linux desktop has espeak or speech-dispatcher, each with its own
//! options. This tool takes the text, a voice and a speed in words per
//! minute on each host.

pub mod plan;
