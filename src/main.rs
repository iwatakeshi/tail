//! A cross-platform implementation of the `tail` command in Rust.
//!
//! Supports all standard GNU `tail` features including:
//! - Line-based and byte-based output
//! - Follow mode (`-f`/`-F`) for monitoring file growth
//! - Multiple file support with headers
//! - Offset-from-start syntax (`+NUM`)
//! - Zero-terminated lines (`-z`)
//!
//! # Examples
//! ```shell
//! $ rtail -n 5 /etc/passwd
//! $ cat /etc/passwd | rtail -n 5
//! $ rtail -c 100 myfile.txt
//! $ rtail -f /var/log/syslog
//! $ rtail -n +5 myfile.txt       # Skip first 4 lines
//! $ rtail file1.txt file2.txt    # Multiple files with headers
//! ```

mod cli;
mod output;
mod tail;

use std::fs::File;
use std::io;
use std::process::ExitCode;

use cli::{CountMode, TailConfig};
use output::{print_header, should_show_headers};
use tail::bytes;
use tail::follow;
use tail::lines;

fn main() -> ExitCode {
    let config = match cli::parse_args() {
        Ok(config) => config,
        Err(e) => {
            eprint!("{e}");
            return ExitCode::FAILURE;
        }
    };

    match run(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rtail: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Main execution logic.
fn run(config: TailConfig) -> io::Result<()> {
    let files = normalize_files(&config.files);
    let show_headers = should_show_headers(files.len(), config.quiet, config.verbose);
    let delimiter = if config.zero_terminated { b'\0' } else { b'\n' };

    // Process initial tail output for each file
    for (i, file_arg) in files.iter().enumerate() {
        if show_headers {
            if i > 0 {
                println!();
            }
            let display_name = if file_arg == "-" {
                "standard input"
            } else {
                file_arg
            };
            print_header(display_name);
        }

        if file_arg == "-" {
            process_stdin(&config.count_mode, delimiter)?;
        } else {
            process_file(file_arg, &config.count_mode, delimiter)?;
        }
    }

    // Enter follow mode if requested
    if let Some(follow_mode) = config.follow {
        // Filter out stdin entries for follow mode (can't follow stdin)
        let follow_files: Vec<String> = files.iter().filter(|f| *f != "-").cloned().collect();

        if !follow_files.is_empty() {
            follow::follow_files(
                &follow_files,
                follow_mode,
                config.sleep_interval,
                config.pid,
                show_headers,
                config.retry,
            )?;
        }
    }

    Ok(())
}

/// Normalize the file list: if empty, use stdin ("-").
fn normalize_files(files: &[String]) -> Vec<String> {
    if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files.to_vec()
    }
}

/// Process a named file according to the count mode.
fn process_file(path: &str, mode: &CountMode, delimiter: u8) -> io::Result<()> {
    let file = File::open(path)
        .map_err(|e| io::Error::new(e.kind(), format!("cannot open '{path}' for reading: {e}")))?;

    match mode {
        CountMode::Lines(n) => lines::tail_lines_seekable(file, *n, delimiter),
        CountMode::Bytes(n) => bytes::tail_bytes_seekable(file, *n),
        CountMode::LinesFromStart(n) => lines::tail_lines_from_start(file, *n, delimiter),
        CountMode::BytesFromStart(n) => bytes::tail_bytes_from_start(file, *n),
    }
}

/// Process stdin according to the count mode.
fn process_stdin(mode: &CountMode, delimiter: u8) -> io::Result<()> {
    let stdin = io::stdin();

    match mode {
        CountMode::Lines(n) => lines::tail_lines_non_seekable(stdin.lock(), *n, delimiter),
        CountMode::Bytes(n) => bytes::tail_bytes_non_seekable(stdin.lock(), *n),
        CountMode::LinesFromStart(n) => lines::tail_lines_from_start(stdin.lock(), *n, delimiter),
        CountMode::BytesFromStart(n) => bytes::tail_bytes_from_start(stdin.lock(), *n),
    }
}
