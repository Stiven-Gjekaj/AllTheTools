<div align="center">
  <a href="README.md"><b>AllTheTools</b></a>
</div>

# Contributing to AllTheTools

Thanks for your interest in AllTheTools.
AllTheTools is one command, `att`, that gives the same small tools on Windows,
on macOS, and on Linux.
Each tool closes a gap: a tool that one operating system has and the other two
do not.
Contributions of all kinds are welcome: bug reports, hardware reports,
documentation fixes, a new tool, and arguments against a decision that is
already made.

The toolchain is Rust, and it needs nothing else.

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

Those three are the gate, and they run on Windows, on macOS and on Linux.
The [README](README.md) says what each tool does.

Turn on the format hook one time in each clone:

    git config core.hooksPath .githooks

## Ways to contribute

- Report a tool that gives a wrong answer on your computer. Name the host.
- Propose a new tool. Say which operating system has it, and which two do not.
- Make a tool use the API of the host in place of a command of the host.
- Improve the documentation.

Before you start significant work, open an issue to agree on the approach.
This costs you one message and can save you a rewritten pull request.

## What makes a tool belong here

A tool belongs in AllTheTools when all of these are true:

- One or two of the three operating systems have the tool, or an equal tool,
  and the others do not have it.
- The tool gives the same command and the same output on all three hosts.
- The tool needs nothing that the host does not supply. It never asks the
  person to install a second tool.

A tool that the three hosts already share, such as `ping`, does not belong
here.

## The rule that comes before all the others

**A test never writes to a real device.**

Two tools write to a block device: `att probe` and, with an option,
`att surface`.
A test suite that touches a block device can destroy the disk of the person who
runs it.
A continuous integration machine has a system disk, and that disk is a block
device.

Write the test against a file or against a model in memory.
Every layer above the device handle accepts a trait object, because that is the
only way to test it.

A pull request that adds a test which opens a path under `/dev`, a
`\\.\PhysicalDrive` path, or any real disk is refused.
This holds even when the test passes on your machine.

## How a host backend gets a test

Each tool that asks the host for data is two kinds of file.

    parse.rs   decides everything and makes no system call.
               It is compiled on ALL THREE hosts, so its tests run three
               times on every change. Everything that can be wrong is here.

    linux.rs   make system calls and decide nothing. Each one turns what the
    macos.rs   host said into plain Rust data and hands it on. Each one is
    windows.rs gated to its own host.

**A pure module must never name a type from `windows-sys`, `libc`,
`core-foundation` or `io-kit-sys`.**
Those crates belong to some hosts and not to all three, so one such name
deletes the tests on the other hosts without a word.

A test builds the state it needs inside itself, from output captured by hand on
a real machine.
It never reads the `sysfs` or the registry of the computer that runs the test,
because that computer changes and the test then passes or fails for a reason
that has nothing to do with the code.

A host file has no unit test.
The gate runs each tool that reads and changes nothing on each host instead.
That proves that the host file compiles, links, finds the host API and returns.

## When a tool calls a command of the host

The API of the host comes first.
Some data has no public API, or the API is very large for the job.
Then the tool can run a command that ships with the operating system, such as
`ioreg` on macOS or PowerShell on Windows.
It asks for machine output, such as a property list or JSON, and never parses
text for a person, because that text changes with the language of the host.

A tool never runs a command that the host does not ship.
The one exception is Linux, where the tool names the package that holds the
command when the command is missing.

## Coding style

- Match the surrounding code. Small, focused functions and clear names beat
  cleverness.
- Each tool lives in its own directory under `crates/allthetools/src/tools/`.
  A tool does not reach into another tool. Shared code goes into a module at
  the top of the crate.
- Add dependencies sparingly, and say in the pull request why the standard
  library or an existing dependency does not do the job.
- Pin each dependency to an exact version. A new version is a commit of its
  own.
- Prefer MIT and Apache 2.0 for a dependency.

## Commit messages and pull requests

[`AGENTS.md`](AGENTS.md) is the full set of rules. These four cause the most
rework.

- **One change per commit, and a feature is many commits.** A commit that says
  "add the full tool" is wrong even when the code is right. Split it into the
  steps that a reviewer can read and revert one at a time.
- **Code and its tests go in one commit. Documentation goes in its own.**
- **All text uses Simplified Technical English.** Short sentences, active voice,
  present tense. No em-dashes and no emoji, in source, comments, documentation,
  commit messages, or pull requests.
- **Write the subject in the present tense, with no version number.** A commit
  changes no version.

In the pull request, describe what changed and why, and say how you tested it.
Name the host you tested on.
"It works" is not a test report, because each tool has a different
implementation on each operating system.

## Reporting security issues

Do not open a public issue for a security problem.
See [SECURITY.md](SECURITY.md) for how to report it privately.

## Code of conduct

By taking part in this project you agree to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).
