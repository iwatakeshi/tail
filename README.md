# tail

A cross-platform `tail` clone written in Rust.

`tail` is a command-line utility that outputs the last N lines (default 10) or bytes from standard input or one or more files. It supports all standard GNU `tail` features including follow mode for monitoring file growth, multiple file support with headers, and offset-from-start syntax.

## Features

- **Line-based output** (`-n`): Output the last N lines (default 10)
- **Byte-based output** (`-c`): Output the last N bytes
- **Offset from start** (`+N`): Output starting from line/byte N with `-n +N` or `-c +N`
- **Follow mode** (`-f`/`-F`): Monitor files for appended data in real time
- **Multiple files**: Process multiple files with automatic headers
- **Header control** (`-q`/`-v`): Suppress or force file name headers
- **Size suffixes**: Support for `K`, `M`, `G`, `kB`, `MB`, `GB`, etc.
- **Zero-terminated** (`-z`): Use NUL as line delimiter instead of newline
- **Process monitoring** (`--pid`): Terminate follow mode when a process dies
- **Log rotation** (`-F`/`--retry`): Handle log file rotation gracefully
- **Cross-platform**: Works on Linux, macOS, and Windows

## Usage Examples

```bash
# Display last 10 lines of a file (default)
rtail /etc/passwd

# Display last 20 lines from stdin
cat /path/to/file | rtail -n 20

# Display last 5 lines of a file
rtail -n 5 /etc/passwd

# Display last 100 bytes
rtail -c 100 myfile.txt

# Display starting from line 5 (skip first 4 lines)
rtail -n +5 myfile.txt

# Display starting from byte 100
rtail -c +100 myfile.txt

# Follow a file for new data (like log monitoring)
rtail -f /var/log/syslog

# Follow with log rotation support
rtail -F /var/log/syslog

# Multiple files with headers
rtail -n 5 file1.txt file2.txt file3.txt

# Use size suffixes
rtail -c 2K myfile.txt    # Last 2 KiB (2048 bytes)
rtail -c 1M myfile.txt    # Last 1 MiB (1048576 bytes)

# Suppress headers with multiple files
rtail -q -n 5 file1.txt file2.txt

# Use NUL-terminated lines
rtail -z -n 5 myfile.txt
```

## Full Usage

```
Print the last 10 lines of each FILE to standard output.
With more than one FILE, precede each with a header giving the file name.

With no FILE, or when FILE is -, read standard input.

Usage: rtail [OPTIONS] [FILE]...

Arguments:
  [FILE]...  Files to read from (optional, reads stdin if omitted or -)

Options:
  -n, --lines <NUM>         Output the last NUM lines; or use -n +NUM to skip NUM-1 lines at the start
  -c, --bytes <NUM>         Output the last NUM bytes; or use -c +NUM to output starting with byte NUM
  -f, --follow [<MODE>]     Output appended data as the file grows
  -F                        Same as --follow=name --retry
      --retry               Keep trying to open a file if it is inaccessible
  -s, --sleep-interval <N>  With -f, sleep for approximately N seconds between iterations [default: 1]
      --pid <PID>           With -f, terminate after process ID PID dies
  -q, --quiet               Never output headers giving file names
  -v, --verbose             Always output headers giving file names
  -z, --zero-terminated     Line delimiter is NUL, not newline
  -h, --help                Print help
  -V, --version             Print version
```

## Size Suffixes

NUM may have a multiplier suffix:
- `b` = 512
- `kB` = 1000, `K` / `KiB` = 1024
- `MB` = 1,000,000, `M` / `MiB` = 1,048,576
- `GB` = 1,000,000,000, `G` / `GiB` = 1,073,741,824
- `TB` = 1,000,000,000,000, `T` / `TiB` = 1,099,511,627,776

## Building

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

## License

Apache License 2.0
