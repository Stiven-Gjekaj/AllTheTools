<div align="center">
  <a href="README.md"><b>AllTheTools</b></a>
</div>

# Terms and Conditions

These terms govern your use of AllTheTools, an open source command line tool
that gives the same small tools on Windows, on macOS and on Linux (the
"Software").
By using, building, running, or contributing to the Software, you agree to
these terms.
If you do not agree, do not use the Software.

## 1. Licence

The Software is licensed under the MIT License, in the [LICENSE](LICENSE) file.
That licence is the authoritative statement of your rights to use, copy,
modify, and distribute the Software.
If anything here conflicts with the LICENSE file, the LICENSE file governs.

## 2. No warranty

The Software is provided "as is", without warranty of any kind, express or
implied.
This includes the warranties of merchantability, fitness for a particular
purpose, and noninfringement.
You use the Software at your own risk.

## 3. Limitation of liability

To the maximum extent that the law permits, the authors and copyright holders
are not liable for any claim, damages, or other liability that arises from the
use of the Software.
This applies whether the claim is in contract, in tort, or otherwise.

## 4. Tools that change data

**Read this section before you run these tools.**

Most tools of the Software only read.
These tools change data or state:

- `att probe` writes to a drive. It reads each block before it writes, and it
  writes the block back at the end. A drive that fails, a cable that comes
  loose, or a stop of the computer during a run can leave the drive without
  that data. A fake drive can also lose data that it already held, because
  that is what makes it fake.
- `att surface --write` writes to each block of a drive, and erases the drive.
- `att eject` and `att mount` change what the system mounts.
- `att whoport --kill` and `att timeout` stop a process.
- `att ddc set` changes a setting of a monitor.
- `att unquarantine` removes the mark that a browser puts on a downloaded file.
  The operating system then starts the file without its warning.

Each of these tools names its target before it acts.
That target is your choice, and it is your responsibility.
Keep a backup of anything that you value before you test a drive.

## 5. The network

`att netquality` sends and receives data through a test server on the internet.
By default that server belongs to Cloudflare, and its own terms apply to it.
`att wol` sends a packet on your local network.
No other tool of the Software uses the network.

## 6. Contributions

If you contribute to the Software, you agree that your contribution carries the
same MIT Licence as the rest of the project.
See [CONTRIBUTING.md](CONTRIBUTING.md) for how to take part.

## 7. Third-party components

The Software uses open source libraries.
Each of those carries its own licence, and that licence governs that component.

## 8. Project name

"AllTheTools" and the `att` command name identify this project.
You may refer to the project by name.
Please do not use the name in a way that suggests endorsement of, or connection
with, a modified or unofficial version, without permission.

## 9. Changes to these terms

These terms may change as the project grows.
The version in the default branch of the repository is the current one.
If you continue to use the Software after a change, you accept the updated
terms.
