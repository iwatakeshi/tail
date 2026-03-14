//! Command-line argument parsing for the `rtail` utility.
//!
//! Supports all standard GNU `tail` options including:
//! - Line-based and byte-based tail operations
//! - Follow mode with descriptor or name tracking
//! - Multiple file support with headers
//! - Offset-from-start syntax (`+NUM`)

use clap::Parser;

/// The mode for counting: lines or bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum CountMode {
    /// Output the last N lines (default behavior).
    Lines(usize),
    /// Output starting from line N (1-indexed, skip N-1 lines).
    LinesFromStart(usize),
    /// Output the last N bytes.
    Bytes(usize),
    /// Output starting from byte N (1-indexed).
    BytesFromStart(usize),
}

/// How to follow a file for new data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FollowMode {
    /// Follow by file descriptor (default for `-f`).
    Descriptor,
    /// Follow by file name (handles log rotation).
    Name,
}

/// Parsed and validated configuration for the tail operation.
#[derive(Debug, Clone)]
pub struct TailConfig {
    /// What and how much to output.
    pub count_mode: CountMode,
    /// Files to read from. Empty means stdin.
    pub files: Vec<String>,
    /// Whether to follow files for appended data.
    pub follow: Option<FollowMode>,
    /// Whether to keep retrying if a file is inaccessible.
    pub retry: bool,
    /// Sleep interval in seconds between follow iterations.
    pub sleep_interval: f64,
    /// Process ID to monitor; terminate follow when it dies.
    pub pid: Option<u32>,
    /// Whether to suppress file name headers.
    pub quiet: bool,
    /// Whether to always show file name headers.
    pub verbose: bool,
    /// Use NUL as line delimiter instead of newline.
    pub zero_terminated: bool,
}

#[derive(Parser)]
#[command(
    name = "rtail",
    version,
    about = "Print the last 10 lines of each FILE to standard output.\nWith more than one FILE, precede each with a header giving the file name.\n\nWith no FILE, or when FILE is -, read standard input."
)]
struct Args {
    /// Output the last NUM lines; or use -n +NUM to skip NUM-1 lines at the start
    #[arg(short = 'n', long = "lines", value_name = "NUM")]
    lines: Option<String>,

    /// Output the last NUM bytes; or use -c +NUM to output starting with byte NUM
    #[arg(short = 'c', long = "bytes", value_name = "NUM")]
    bytes: Option<String>,

    /// Output appended data as the file grows
    #[arg(short = 'f', long = "follow", value_name = "MODE", num_args = 0..=1, default_missing_value = "descriptor")]
    follow: Option<String>,

    /// Same as --follow=name --retry
    #[arg(short = 'F')]
    follow_retry: bool,

    /// Keep trying to open a file if it is inaccessible
    #[arg(long = "retry")]
    retry: bool,

    /// With -f, sleep for approximately N seconds between iterations
    #[arg(
        short = 's',
        long = "sleep-interval",
        value_name = "N",
        default_value_t = 1.0
    )]
    sleep_interval: f64,

    /// With -f, terminate after process ID PID dies
    #[arg(long = "pid", value_name = "PID")]
    pid: Option<u32>,

    /// Never output headers giving file names
    #[arg(short = 'q', long = "quiet", alias = "silent")]
    quiet: bool,

    /// Always output headers giving file names
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Line delimiter is NUL, not newline
    #[arg(short = 'z', long = "zero-terminated")]
    zero_terminated: bool,

    /// Files to read from (optional, reads stdin if omitted or -)
    #[arg(value_name = "FILE")]
    files: Vec<String>,
}

/// Parse a number string that may have a `+` prefix (meaning from-start)
/// and optional suffixes (b, kB, K, MB, M, GB, G, etc.).
///
/// Returns `(value, from_start)` where `from_start` is true if the number
/// was prefixed with `+`.
fn parse_number(s: &str) -> Result<(usize, bool), String> {
    let s = s.trim();
    let (from_start, s) = if let Some(rest) = s.strip_prefix('+') {
        (true, rest)
    } else {
        let s = s.strip_prefix('-').unwrap_or(s);
        (false, s)
    };

    // Split into digits and suffix
    let digit_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());

    let (num_str, suffix) = s.split_at(digit_end);

    if num_str.is_empty() {
        return Err(format!("invalid number: '{s}'"));
    }

    let base: usize = num_str
        .parse()
        .map_err(|_| format!("invalid number: '{num_str}'"))?;

    let multiplier: usize = match suffix {
        "" => 1,
        "b" => 512,
        "kB" => 1000,
        "K" | "KiB" => 1024,
        "MB" => 1_000_000,
        "M" | "MiB" => 1_048_576,
        "GB" => 1_000_000_000,
        "G" | "GiB" => 1_073_741_824,
        "TB" => 1_000_000_000_000,
        "T" | "TiB" => 1_099_511_627_776,
        _ => return Err(format!("invalid suffix: '{suffix}'")),
    };

    base.checked_mul(multiplier)
        .ok_or_else(|| format!("number too large: '{s}'"))
        .map(|v| (v, from_start))
}

/// Pre-process raw arguments to handle the legacy `-NUM` shorthand (e.g. `tail -30`),
/// translating it into `-n NUM` before clap sees the argument list.
///
/// Any argument of the form `-<digits>` where all characters after the dash are ASCII
/// digits is rewritten as two arguments: `-n` and `<digits>`. Everything after a bare
/// `--` separator is passed through unchanged.
fn preprocess_args<I, T>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let mut result: Vec<std::ffi::OsString> = Vec::new();
    let mut iter = args.into_iter();

    // First argument is the program name — pass through as-is.
    if let Some(prog) = iter.next() {
        result.push(prog.into());
    }

    let mut end_of_options = false;
    for arg in iter {
        let os_arg: std::ffi::OsString = arg.into();
        if end_of_options {
            result.push(os_arg);
            continue;
        }
        if os_arg == "--" {
            end_of_options = true;
            result.push(os_arg);
            continue;
        }
        // Detect `-NUM` (e.g. `-30`) and expand to `-n` + `30`.
        if let Some(s) = os_arg.to_str() {
            if let Some(num_str) = s.strip_prefix('-') {
                if !num_str.is_empty() && num_str.bytes().all(|b| b.is_ascii_digit()) {
                    result.push(std::ffi::OsString::from("-n"));
                    result.push(std::ffi::OsString::from(num_str));
                    continue;
                }
            }
        }
        result.push(os_arg);
    }
    result
}

/// Parse command-line arguments into a validated `TailConfig`.
pub fn parse_args() -> Result<TailConfig, String> {
    parse_args_from(std::env::args_os())
}

/// Parse arguments from an iterator (useful for testing).
pub fn parse_args_from<I, T>(args: I) -> Result<TailConfig, String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let preprocessed = preprocess_args(args);
    let args = Args::try_parse_from(preprocessed).map_err(|e| e.to_string())?;

    // Validate: -c and -n are mutually exclusive
    if args.lines.is_some() && args.bytes.is_some() {
        return Err("cannot use both --lines and --bytes".to_string());
    }

    // Determine count mode
    let count_mode = if let Some(ref bytes_str) = args.bytes {
        let (count, from_start) = parse_number(bytes_str)?;
        if from_start {
            CountMode::BytesFromStart(count)
        } else {
            CountMode::Bytes(count)
        }
    } else if let Some(ref lines_str) = args.lines {
        let (count, from_start) = parse_number(lines_str)?;
        if from_start {
            CountMode::LinesFromStart(count)
        } else {
            CountMode::Lines(count)
        }
    } else {
        CountMode::Lines(10)
    };

    // Determine follow mode
    let follow = if args.follow_retry {
        Some(FollowMode::Name)
    } else if let Some(ref mode) = args.follow {
        match mode.as_str() {
            "descriptor" | "d" => Some(FollowMode::Descriptor),
            "name" | "n" => Some(FollowMode::Name),
            _ => return Err(format!("invalid follow mode: '{mode}'")),
        }
    } else {
        None
    };

    let retry = args.retry || args.follow_retry;

    Ok(TailConfig {
        count_mode,
        files: args.files,
        follow,
        retry,
        sleep_interval: args.sleep_interval,
        pid: args.pid,
        quiet: args.quiet,
        verbose: args.verbose,
        zero_terminated: args.zero_terminated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<TailConfig, String> {
        let mut full_args = vec!["rtail"];
        full_args.extend_from_slice(args);
        parse_args_from(full_args)
    }

    #[test]
    fn test_default_lines() {
        let config = parse(&[]).unwrap();
        assert_eq!(config.count_mode, CountMode::Lines(10));
    }

    #[test]
    fn test_lines_flag() {
        let config = parse(&["-n", "5"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Lines(5));
    }

    #[test]
    fn test_lines_from_start() {
        let config = parse(&["-n", "+3"]).unwrap();
        assert_eq!(config.count_mode, CountMode::LinesFromStart(3));
    }

    #[test]
    fn test_bytes_flag() {
        let config = parse(&["-c", "100"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Bytes(100));
    }

    #[test]
    fn test_bytes_from_start() {
        let config = parse(&["-c", "+1"]).unwrap();
        assert_eq!(config.count_mode, CountMode::BytesFromStart(1));
    }

    #[test]
    fn test_bytes_with_suffix() {
        let config = parse(&["-c", "2K"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Bytes(2048));
    }

    #[test]
    fn test_follow_default() {
        let config = parse(&["-f"]).unwrap();
        assert_eq!(config.follow, Some(FollowMode::Descriptor));
    }

    #[test]
    fn test_follow_name() {
        let config = parse(&["--follow=name"]).unwrap();
        assert_eq!(config.follow, Some(FollowMode::Name));
    }

    #[test]
    fn test_follow_retry() {
        let config = parse(&["-F"]).unwrap();
        assert_eq!(config.follow, Some(FollowMode::Name));
        assert!(config.retry);
    }

    #[test]
    fn test_quiet_flag() {
        let config = parse(&["-q"]).unwrap();
        assert!(config.quiet);
    }

    #[test]
    fn test_verbose_flag() {
        let config = parse(&["-v"]).unwrap();
        assert!(config.verbose);
    }

    #[test]
    fn test_zero_terminated() {
        let config = parse(&["-z"]).unwrap();
        assert!(config.zero_terminated);
    }

    #[test]
    fn test_multiple_files() {
        let config = parse(&["file1.txt", "file2.txt"]).unwrap();
        assert_eq!(config.files, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_lines_and_bytes_conflict() {
        let result = parse(&["-n", "5", "-c", "10"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_legacy_num_shorthand() {
        let config = parse(&["-30"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Lines(30));
    }

    #[test]
    fn test_legacy_num_shorthand_single_digit() {
        let config = parse(&["-5"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Lines(5));
    }

    #[test]
    fn test_legacy_num_shorthand_with_file() {
        let config = parse(&["-20", "somefile.txt"]).unwrap();
        assert_eq!(config.count_mode, CountMode::Lines(20));
        assert_eq!(config.files, vec!["somefile.txt"]);
    }

    #[test]
    fn test_legacy_num_does_not_affect_named_flags() {
        // -f should not be rewritten — it is not all digits
        let config = parse(&["-f"]).unwrap();
        assert_eq!(config.follow, Some(FollowMode::Descriptor));
    }

    #[test]
    fn test_pid_flag() {
        let config = parse(&["-f", "--pid", "1234"]).unwrap();
        assert_eq!(config.pid, Some(1234));
    }

    #[test]
    fn test_sleep_interval() {
        let config = parse(&["-f", "-s", "2.5"]).unwrap();
        assert!((config.sleep_interval - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_number_suffixes() {
        assert_eq!(parse_number("10").unwrap(), (10, false));
        assert_eq!(parse_number("+5").unwrap(), (5, true));
        assert_eq!(parse_number("-3").unwrap(), (3, false));
        assert_eq!(parse_number("1b").unwrap(), (512, false));
        assert_eq!(parse_number("1kB").unwrap(), (1000, false));
        assert_eq!(parse_number("1K").unwrap(), (1024, false));
        assert_eq!(parse_number("1M").unwrap(), (1_048_576, false));
        assert_eq!(parse_number("1MB").unwrap(), (1_000_000, false));
        assert_eq!(parse_number("1G").unwrap(), (1_073_741_824, false));
    }
}
