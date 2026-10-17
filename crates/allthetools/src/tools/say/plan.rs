//! The speech program of each host, and the speed of the voice in the unit
//! that each one takes.
//!
//! The person gives the speed in words per minute, as `say` of macOS and
//! `espeak` take it. speech-dispatcher and the speech of Windows take a
//! scale, and the tool turns the words into a step of that scale.
//!
//! This module makes no system call, so its tests run on every host.

/// The normal speed of the voices, in words per minute.
pub const NORMAL_WPM: u32 = 180;

/// The speed of Windows: -10 to 10, where 0 is the normal speed.
pub fn sapi_rate(wpm: u32) -> i32 {
    ((f64::from(wpm) - f64::from(NORMAL_WPM)) / 18.0)
        .round()
        .clamp(-10.0, 10.0) as i32
}

/// The speed of speech-dispatcher: -100 to 100, where 0 is the normal
/// speed.
pub fn dispatcher_rate(wpm: u32) -> i32 {
    ((f64::from(wpm) - f64::from(NORMAL_WPM)) / 2.0)
        .round()
        .clamp(-100.0, 100.0) as i32
}

/// The speech programs of Linux, in the order that the tool tries them.
pub const LINUX: [&str; 3] = ["espeak-ng", "espeak", "spd-say"];

/// The first speech program of Linux that the host has.
pub fn linux_program(on_path: impl Fn(&str) -> bool) -> Option<&'static str> {
    LINUX.into_iter().find(|p| on_path(p))
}

/// The arguments of a Linux speech program. espeak reads the text from its
/// input stream, and spd-say takes it as an argument.
pub fn linux_args(program: &str, voice: Option<&str>, wpm: Option<u32>, text: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if program == "spd-say" {
        args.push("--wait".to_owned());
        if let Some(wpm) = wpm {
            args.extend(["--rate".to_owned(), dispatcher_rate(wpm).to_string()]);
        }
        if let Some(voice) = voice {
            args.extend(["--synthesis-voice".to_owned(), voice.to_owned()]);
        }
        args.extend(["--".to_owned(), text.to_owned()]);
    } else {
        if let Some(wpm) = wpm {
            args.extend(["-s".to_owned(), wpm.to_string()]);
        }
        if let Some(voice) = voice {
            args.extend(["-v".to_owned(), voice.to_owned()]);
        }
        args.push("--stdin".to_owned());
    }
    args
}

/// The PowerShell script of Windows. It reads the text, the voice and the
/// speed from the environment, so no text of the person goes into the
/// script.
pub const POWERSHELL: &str = r"
Add-Type -AssemblyName System.Speech
$s = New-Object System.Speech.Synthesis.SpeechSynthesizer
if ($env:ATT_SAY_LIST) {
    $s.GetInstalledVoices() | Where-Object Enabled | ForEach-Object { $_.VoiceInfo.Name }
    return
}
if ($env:ATT_SAY_VOICE) { $s.SelectVoice($env:ATT_SAY_VOICE) }
if ($env:ATT_SAY_RATE) { $s.Rate = [int]$env:ATT_SAY_RATE }
$s.Speak($env:ATT_SAY_TEXT)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_normal_speed_is_zero_on_each_scale() {
        assert_eq!(sapi_rate(NORMAL_WPM), 0);
        assert_eq!(dispatcher_rate(NORMAL_WPM), 0);
    }

    #[test]
    fn a_speed_stays_inside_each_scale() {
        assert_eq!(sapi_rate(1000), 10);
        assert_eq!(sapi_rate(0), -10);
        assert_eq!(sapi_rate(270), 5);
        assert_eq!(dispatcher_rate(1000), 100);
        assert_eq!(dispatcher_rate(80), -50);
    }

    #[test]
    fn linux_takes_the_first_program_it_has() {
        assert_eq!(linux_program(|p| p != "espeak-ng"), Some("espeak"));
        assert_eq!(linux_program(|_| false), None);
    }

    #[test]
    fn espeak_reads_the_text_from_its_input_and_spd_say_from_an_argument() {
        let espeak = linux_args("espeak-ng", Some("en"), Some(200), "hello");
        assert_eq!(espeak, ["-s", "200", "-v", "en", "--stdin"]);
        let spd = linux_args("spd-say", None, None, "-n hello");
        assert_eq!(spd, ["--wait", "--", "-n hello"]);
    }
}
