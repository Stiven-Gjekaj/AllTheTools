//! `att clip`: copy text to the clipboard, and paste it.
//!
//! macOS has pbcopy and pbpaste. Windows has `clip`, which only copies.
//! Linux has a different program for each display server, and a desktop
//! often has none of them. This tool is one command on each host.

use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, Result};

pub mod parse;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

/// Copy text to the clipboard, and paste it.
///
/// With no action, att clip copies its input when the input is not a
/// terminal, and pastes when it is.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub action: Option<Action>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Action {
    /// Copy the text to the clipboard. With no text, copy the input.
    Copy {
        /// The text. The words go in with one space between them.
        text: Vec<String>,
    },
    /// Write the text of the clipboard on the output.
    Paste,
}

pub fn run(args: Args) -> Result<i32> {
    let action = args.action.unwrap_or_else(|| {
        if std::io::stdin().is_terminal() {
            Action::Paste
        } else {
            Action::Copy { text: Vec::new() }
        }
    });
    match action {
        Action::Copy { text } => {
            let text = if text.is_empty() {
                let mut input = String::new();
                std::io::stdin()
                    .read_to_string(&mut input)
                    .context("the input is not UTF-8 text")?;
                input
            } else {
                text.join(" ")
            };
            host::copy(&text)?;
        }
        Action::Paste => {
            let text = host::paste()?;
            let mut out = std::io::stdout();
            out.write_all(text.as_bytes())?;
            out.flush()?;
        }
    }
    Ok(0)
}
