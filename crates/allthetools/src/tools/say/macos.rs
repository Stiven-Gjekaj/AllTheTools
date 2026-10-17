//! The host file of `att say` on macOS: `say`, which reads the text from its
//! input stream.

use anyhow::Result;

use crate::host;

pub fn say(text: &str, voice: Option<&str>, wpm: Option<u32>) -> Result<()> {
    let wpm = wpm.map(|w| w.to_string());
    let mut args = Vec::new();
    if let Some(voice) = voice {
        args.extend(["-v", voice]);
    }
    if let Some(wpm) = &wpm {
        args.extend(["-r", wpm.as_str()]);
    }
    host::feed("say", &args, text.as_bytes(), None)
}

pub fn voices() -> Result<String> {
    host::text("say", &["-v", "?"], None)
}
