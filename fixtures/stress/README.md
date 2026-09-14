# Stress and incomplete evidence

`cases.json` includes Unicode and long logical source paths, empty commit
subjects, corrupt syntax/regex/report inputs, missing rule prerequisites and
foreign snapshot facts. Each built-in rule retains compliant/violating cases.

Native fixtures create isolated temporary Git repositories and exercise a
physical path exceeding 260 characters with spaces and non-ASCII text, a file
over 2 MiB, an index symlink, a missing loose Git object, and changed execution
inputs. Git's index symlink mode is portable and needs no Windows symlink
privilege. Runner fixtures exercise a real timeout, output over 16 MiB and a
missing executable. The temporary repositories are removed after execution.

These native checks run on the current host. They do not simulate other OSes,
network filesystems or every Windows ACL/sharing failure. Missing host
capabilities produce incomplete evidence, not silent skips. CI runs the same
corpus on Linux, Windows and macOS.

See [the stress golden](../golden/stress.json) for the expected failure reasons.
