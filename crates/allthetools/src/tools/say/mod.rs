//! `att say`: read text aloud with a voice of the host.
//!
//! macOS has `say`. Windows has voices and no command that uses them, and a
//! Linux desktop has espeak or speech-dispatcher, each with its own
//! options. This tool takes the text, a voice and a speed in words per
//! minute on each host.

use std::io::Read;

use anyhow::{Result, bail};

pub mod plan;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
use windows as host;

/// Read text aloud with a voice of the host.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The text. With none, the input stream.
    pub text: Vec<String>,

    /// The name of the voice.
    #[arg(short, long)]
    pub voice: Option<String>,

    /// The speed, in words per minute. 180 is the normal speed.
    #[arg(short, long, value_name = "WPM", value_parser = clap::value_parser!(u32).range(40..=600))]
    pub rate: Option<u32>,

    /// List the voices of the host.
    #[arg(short, long, conflicts_with_all = ["text", "voice", "rate"])]
    pub list: bool,
}

pub fn run(args: Args) -> Result<i32> {
    if args.list {
        print!("{}", host::voices()?);
        return Ok(0);
    }
    let text = if args.text.is_empty() {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        text
    } else {
        args.text.join(" ")
    };
    if text.trim().is_empty() {
        bail!("there is no text to say");
    }
    host::say(&text, args.voice.as_deref(), args.rate)?;
    Ok(0)
}
