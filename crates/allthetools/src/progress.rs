//! A line of progress on the error stream, for a long task.
//!
//! The line shows only when the error stream is a terminal, so a log or a
//! script gets no control characters.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

pub struct Progress {
    what: String,
    started: Instant,
    last: Option<Instant>,
    shown: bool,
    terminal: bool,
}

impl Progress {
    pub fn new(what: &str) -> Self {
        Progress {
            what: what.to_owned(),
            started: Instant::now(),
            last: None,
            shown: false,
            terminal: std::io::stderr().is_terminal(),
        }
    }

    /// Show `done` of `total` bytes, at most two times each second.
    pub fn update(&mut self, done: u64, total: u64) {
        if !self.terminal {
            return;
        }
        let now = Instant::now();
        if self
            .last
            .is_some_and(|last| now - last < Duration::from_millis(500))
            && done < total
        {
            return;
        }
        self.last = Some(now);
        let seconds = (now - self.started).as_secs_f64().max(0.001);
        let percent = if total == 0 {
            100.0
        } else {
            done as f64 * 100.0 / total as f64
        };
        eprint!(
            "\r{}: {percent:5.1}%  {} of {}  {}/s   ",
            self.what,
            crate::size::decimal(done),
            crate::size::decimal(total),
            crate::size::decimal((done as f64 / seconds) as u64)
        );
        let _ = std::io::stderr().flush();
        self.shown = true;
    }

    /// End the line, so the next text starts on a line of its own.
    pub fn finish(&mut self) {
        if self.shown {
            eprintln!();
            self.shown = false;
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}
