//! The host file of `att say` on Windows: the SpeechSynthesizer of
//! System.Speech, through PowerShell.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::plan;

fn run(env: &[(&str, String)]) -> Result<String> {
    let script = format!("$ErrorActionPreference = 'Stop'; {}", plan::POWERSHELL);
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    let out = command.output().context("could not start PowerShell")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bail!("Windows did not speak: {}", first.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn say(text: &str, voice: Option<&str>, wpm: Option<u32>) -> Result<()> {
    let mut env = vec![("ATT_SAY_TEXT", text.to_owned())];
    if let Some(voice) = voice {
        env.push(("ATT_SAY_VOICE", voice.to_owned()));
    }
    if let Some(wpm) = wpm {
        env.push(("ATT_SAY_RATE", plan::sapi_rate(wpm).to_string()));
    }
    run(&env)?;
    Ok(())
}

pub fn voices() -> Result<String> {
    run(&[("ATT_SAY_LIST", "1".to_owned())])
}
