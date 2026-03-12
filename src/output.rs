//! Output formatting for the `rtail` utility.
//!
//! Handles file name headers and output writing.

use std::io::{self, Write};

/// Prints the standard file header: `==> filename <==`
pub fn print_header(filename: &str) {
    // Use writeln to stderr-safe stdout
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "==> {filename} <==");
}

/// Determines whether headers should be shown based on file count and flags.
pub fn should_show_headers(file_count: usize, quiet: bool, verbose: bool) -> bool {
    if quiet {
        return false;
    }
    if verbose {
        return true;
    }
    // Default: show headers when there are multiple files
    file_count > 1
}
