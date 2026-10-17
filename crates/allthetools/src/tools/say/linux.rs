//! The host file of `att say` on Linux: espeak-ng, espeak or spd-say,
//! whichever the host has first.

use anyhow::{Result, bail};

use super::plan;
use crate::host;

fn program() -> Result<&'static str> {
    match plan::linux_program(host::on_path) {
        Some(program) => Ok(program),
        None => bail!(
            "this host has no espeak-ng, espeak or spd-say. Install espeak-ng, or speech-dispatcher"
        ),
    }
}

pub fn say(text: &str, voice: Option<&str>, wpm: Option<u32>, output: Option<&str>) -> Result<()> {
    let program = program()?;
    if output.is_some() && program == "spd-say" {
        bail!("spd-say cannot write a file. Install espeak-ng to write the speech to a file");
    }
    let args = plan::linux_args(program, voice, wpm, output, text);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if program == "spd-say" {
        host::output(program, &args, None)?;
        Ok(())
    } else {
        host::feed(program, &args, text.as_bytes(), None)
    }
}

pub fn voices() -> Result<String> {
    let program = program()?;
    let flag = if program == "spd-say" {
        "--list-synthesis-voices"
    } else {
        "--voices"
    };
    host::text(program, &[flag], None)
}
