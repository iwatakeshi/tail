//! Follow mode for tailing files as they grow.
//!
//! Supports two modes:
//! - **Descriptor**: Follow the open file descriptor (default `-f`).
//! - **Name**: Follow the file by name, handling rotation (`-F`/`--follow=name`).

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::thread;
use std::time::Duration;

use crate::cli::FollowMode;

/// Follow one or more files for appended data.
///
/// This function runs indefinitely (until the process is killed or
/// the monitored PID dies) and outputs new data as it appears.
pub fn follow_files(
    files: &[String],
    mode: FollowMode,
    sleep_interval: f64,
    pid: Option<u32>,
    show_headers: bool,
    retry: bool,
) -> io::Result<()> {
    let sleep_duration = Duration::from_secs_f64(sleep_interval);

    let mut states: Vec<FollowState> = files
        .iter()
        .map(|path| FollowState::new(path.clone(), retry))
        .collect();

    // Initialize: seek to end of each file
    for state in &mut states {
        state.open_and_seek_to_end();
    }

    let multiple_files = files.len() > 1;
    let mut last_printed_index: Option<usize> = None;

    loop {
        // Check if the monitored process is still alive
        if let Some(pid) = pid {
            if !is_process_alive(pid) {
                return Ok(());
            }
        }

        for (i, state) in states.iter_mut().enumerate() {
            let has_new_data = match mode {
                FollowMode::Descriptor => state.check_for_new_data()?,
                FollowMode::Name => state.check_for_new_data_by_name()?,
            };

            if has_new_data {
                if show_headers && (multiple_files || last_printed_index != Some(i)) {
                    let stdout = io::stdout();
                    let mut out = stdout.lock();
                    if last_printed_index.is_some() {
                        let _ = writeln!(out);
                    }
                    let _ = writeln!(out, "==> {} <==", state.path);
                }
                state.flush_new_data()?;
                last_printed_index = Some(i);
            }
        }

        thread::sleep(sleep_duration);
    }
}

/// State for following a single file.
struct FollowState {
    path: String,
    file: Option<File>,
    position: u64,
    retry: bool,
    #[cfg(unix)]
    inode: Option<u64>,
}

impl FollowState {
    fn new(path: String, retry: bool) -> Self {
        Self {
            path,
            file: None,
            position: 0,
            retry,
            #[cfg(unix)]
            inode: None,
        }
    }

    fn open_and_seek_to_end(&mut self) {
        match File::open(&self.path) {
            Ok(mut f) => {
                if let Ok(pos) = f.seek(SeekFrom::End(0)) {
                    self.position = pos;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if let Ok(meta) = f.metadata() {
                            self.inode = Some(meta.ino());
                        }
                    }
                    self.file = Some(f);
                }
            }
            Err(e) => {
                if self.retry {
                    eprintln!("rtail: cannot open '{}' for reading: {e}", self.path);
                }
            }
        }
    }

    /// Check if there is new data available via file descriptor.
    /// If so, seek to the position where new data starts.
    fn check_for_new_data(&mut self) -> io::Result<bool> {
        if self.file.is_none() {
            if self.retry {
                self.try_reopen()?;
            }
            return Ok(false);
        }

        let file = self.file.as_mut().unwrap();
        let metadata = file.metadata()?;
        let current_size = metadata.len();

        if current_size < self.position {
            // File was truncated
            eprintln!("rtail: {}: file truncated", self.path);
            self.position = 0;
        }

        if current_size > self.position {
            file.seek(SeekFrom::Start(self.position))?;
            self.position = current_size;
            return Ok(true);
        }

        Ok(false)
    }

    /// Check for new data by name - detect file rotation and reopen as needed.
    fn check_for_new_data_by_name(&mut self) -> io::Result<bool> {
        if self.has_file_been_replaced() {
            eprintln!(
                "rtail: '{}' has been replaced; following new file",
                self.path
            );
            self.file = None;
            self.position = 0;
            self.try_reopen()?;

            if let Some(ref mut file) = self.file {
                let current_size = file.metadata()?.len();
                if current_size > 0 {
                    file.seek(SeekFrom::Start(0))?;
                    self.position = current_size;
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        self.check_for_new_data()
    }

    /// Check if the file on disk has changed identity (different inode on Unix).
    fn has_file_been_replaced(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Some(current_inode) = self.inode {
                match fs::metadata(&self.path) {
                    Ok(meta) => return meta.ino() != current_inode,
                    Err(_) => return true, // File disappeared
                }
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix: check if file disappeared and reappeared (size reset)
            if self.file.is_some() {
                match fs::metadata(&self.path) {
                    Ok(meta) => return meta.len() < self.position,
                    Err(_) => return true,
                }
            }
        }

        false
    }

    /// Try to reopen the file.
    fn try_reopen(&mut self) -> io::Result<()> {
        match File::open(&self.path) {
            Ok(f) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(meta) = f.metadata() {
                        self.inode = Some(meta.ino());
                    }
                }
                self.file = Some(f);
                self.position = 0;
            }
            Err(_) if self.retry => {
                // Will retry on next iteration
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Read and output new data from the current position to stdout.
    fn flush_new_data(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.file {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let mut buf = [0u8; 4096];

            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
            }
            out.flush()?;
        }
        Ok(())
    }
}

/// Check if a process with the given PID is still alive.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Check /proc on Linux; fall back to kill -0 on other Unix
        let proc_path = format!("/proc/{pid}");
        if std::path::Path::new("/proc").exists() {
            return std::path::Path::new(&proc_path).exists();
        }
        // Fallback for macOS/BSD: use kill -0
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        // Use tasklist to check if process exists
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|output| {
                let s = String::from_utf8_lossy(&output.stdout);
                s.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        true
    }
}
