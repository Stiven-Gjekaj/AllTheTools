<div align="center">
  <a href="README.md"><b>AllTheTools</b></a>
</div>

# Security Policy

## Supported versions

Fixes go to the latest release and to the default branch.
Older versions are not maintained.

## Reporting a vulnerability

Report security problems privately, and not through a public issue.

- Preferred: open a private security advisory with the "Report a vulnerability"
  button on the Security tab of this repository.
- Alternative: email the maintainer at stivenagostingjekaj@gmail.com.

Include the steps to reproduce, the affected commit, and the impact as you
understand it.
You can expect a first answer within a few days.
Your report gets an acknowledgement when the fix ships, unless you prefer to
stay anonymous.

## The threat model

AllTheTools is twenty small tools in one binary. Most of them only read.
Read this before you report.

**Some tools need high privilege.**
`att probe` and `att surface` open a raw device, so they need root rights on
macOS and on Linux, and administrator rights on Windows.
`att whoholds` and `att whoport` see more processes with high privilege.
That is the job, and it is not a defect.
The question that matters is what the tool does with those rights.

**The worst outcome is a write to the wrong device.**
`att probe` writes to a drive, and it puts back what it wrote over.
A bug that selects the wrong target, or that does not put the data back,
destroys data that the person did not choose.
This is the highest severity problem that this project can have.

**A system disk is never a target.**
`att probe` refuses the disk that the running system starts from.
A way around that refusal is a vulnerability, not a feature request.

**The input is untrusted.**
`att ldd` reads a binary that can come from the internet.
`att netquality` reads answers from a server on the network.
A parser that a crafted input can crash, hang, or make read outside its buffer
is in scope.

**These are in scope:**

- Any path that writes to a device that the person did not select and confirm.
- Any way to defeat the refusal to write to a system disk.
- A `probe` run that does not put back the data that it wrote over, when it
  reports that it did.
- A parser that a crafted input can crash, hang, or make read or write outside
  its buffer.
- A tool that stops a process that the person did not name and confirm.
- A temporary file or a link that a local attacker can replace between the
  check and the use.

**These are out of scope:**

- A report that a tool needs administrator or root rights.
- A report that `att unquarantine` removes the download mark of a file. That
  is what it does, and the person names the file.
- Findings from an automated scanner with no working demonstration.
