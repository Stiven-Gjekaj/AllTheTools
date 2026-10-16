//! The text that each host needs for a notification.
//!
//! The title and the message go to each host as data, never as part of a
//! script, so a quote in a message cannot change what the script does. This
//! module makes no system call, so its tests run on every host.

/// The AppleScript of macOS. It reads the message and the title from its
/// arguments.
pub const APPLESCRIPT: [&str; 3] = [
    "on run argv",
    "display notification (item 1 of argv) with title (item 2 of argv)",
    "end run",
];

/// The PowerShell script of Windows. It reads the title and the message from
/// the environment, and shows a toast as Windows PowerShell, which has an
/// app ID that Windows knows.
pub const POWERSHELL: &str = r"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$text = $xml.GetElementsByTagName('text')
$text.Item(0).AppendChild($xml.CreateTextNode($env:ATT_NOTIFY_TITLE)) | Out-Null
$text.Item(1).AppendChild($xml.CreateTextNode($env:ATT_NOTIFY_MESSAGE)) | Out-Null
$app = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($app).Show([Windows.UI.Notifications.ToastNotification]::new($xml))
";

/// A string in the text form of a GVariant, which `gdbus call` reads: in
/// single quotes, with a backslash before each quote and each backslash.
pub fn gvariant_string(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        match c {
            '\'' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// The arguments of `gdbus call` for the Notify method of the notification
/// server of a Linux desktop.
pub fn gdbus_args(title: &str, message: &str) -> Vec<String> {
    [
        "call",
        "--session",
        "--dest",
        "org.freedesktop.Notifications",
        "--object-path",
        "/org/freedesktop/Notifications",
        "--method",
        "org.freedesktop.Notifications.Notify",
        "'att'",
        "0",
        "''",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([gvariant_string(title), gvariant_string(message)])
    // The timeout is written with its type, because gdbus reads an argument
    // that starts with a hyphen as an option.
    .chain(["[]", "{}", "int32 -1"].iter().map(|s| (*s).to_owned()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quote_cannot_end_the_gvariant_string() {
        assert_eq!(gvariant_string("it's"), r"'it\'s'");
        assert_eq!(gvariant_string(r"a\b"), r"'a\\b'");
        assert_eq!(gvariant_string("two\nlines"), r"'two\nlines'");
    }

    #[test]
    fn gdbus_gets_the_title_and_the_message_in_their_places() {
        let args = gdbus_args("Build", "done");
        assert_eq!(args[11], "'Build'");
        assert_eq!(args[12], "'done'");
        assert!(
            args.iter()
                .all(|a| !a.starts_with('-') || a.starts_with("--"))
        );
        assert_eq!(args.last().map(String::as_str), Some("int32 -1"));
    }

    #[test]
    fn the_scripts_read_their_text_as_data() {
        assert!(APPLESCRIPT[1].contains("item 1 of argv"));
        assert!(POWERSHELL.contains("$env:ATT_NOTIFY_MESSAGE"));
    }
}
