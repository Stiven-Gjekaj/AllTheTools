<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# Stability

From version 1.0, AllTheTools follows [Semantic Versioning](https://semver.org).
This file says what a version promises, so a script can depend on it.

## What stays the same in each 1.x release

- **The name of each tool**, such as `att lsusb`.
- **Each option** of each tool: its long name, its short name, and what it
  does.
- **The exit codes**: 0, 1, 2 and 124, as the
  [README](../README.md#for-a-script) gives them.
- **The JSON output**: the name and the type of each field, and each word
  that a field can hold, as [docs/json.md](json.md) gives them. The tests in
  `crates/allthetools/src/contract.rs` hold the same names.
- **The familiar names** that `att link` makes, and the tool that each one
  runs.

## What a 1.x release can add

- A new tool.
- A new option of a tool.
- A new field in a JSON record. A script must skip a field that it does not
  know.
- A new familiar name for `att link`.
- A new word in a field whose host can give other words, such as the `state`
  of `att whoport`, or the `security` of `att wifi`.

## What can change in any release

- **The text for a person**: the layout of a table, the width of a column,
  the words of a message, and the text of an error. A script reads the JSON,
  and never the text for a person.
- **The order of the rows** where docs/json.md gives no order.
- **The source of the data on a host**, such as an API in place of a command
  of the host, when the output stays the same. Version 0.5 did this for
  `att lsusb` and `att lspci` on Windows.
- **A value that was wrong.** A fix that makes a value correct is a fix, and
  not a change of the contract.

## What needs version 2.0

- A tool, an option or a familiar name that goes away.
- An option that does something else.
- A JSON field that goes away, or that changes its name or its type.
- An exit code that changes its meaning.

A change of this kind first gets a warning on the error stream for at least
one minor release, and a line in [CHANGELOG.md](../CHANGELOG.md).

## The hosts

Each release builds and tests on the current version of each host that
GitHub Actions gives: Ubuntu, macOS on Apple silicon, and Windows. A host that
its maker no longer supports can stop working in a minor release.
[docs/limits.md](limits.md) says what each tool cannot do on each host.
