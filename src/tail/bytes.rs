//! Byte-based tail operations.
//!
//! Supports reading the last N bytes from seekable files and non-seekable
//! streams, as well as reading from a starting byte offset.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};

const BUFFER_SIZE: usize = 4096;

/// Output the last `num_bytes` bytes from a seekable file.
pub fn tail_bytes_seekable(mut file: File, num_bytes: usize) -> io::Result<()> {
    if num_bytes == 0 {
        return Ok(());
    }

    let file_size = file.seek(SeekFrom::End(0))?;
    if file_size == 0 {
        return Ok(());
    }

    // Determine how far back to seek
    let offset = (num_bytes as u64).min(file_size);
    file.seek(SeekFrom::End(-(offset as i64)))?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut buf = [0u8; BUFFER_SIZE];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
    }

    Ok(())
}

/// Output the last `num_bytes` bytes from a non-seekable stream.
pub fn tail_bytes_non_seekable<R: Read>(reader: R, num_bytes: usize) -> io::Result<()> {
    if num_bytes == 0 {
        return Ok(());
    }

    let mut circular_buffer = vec![0u8; num_bytes];
    let mut total_written = 0usize;
    let mut buf_reader = BufReader::new(reader);
    let mut read_buf = [0u8; BUFFER_SIZE];

    loop {
        let n = buf_reader.read(&mut read_buf)?;
        if n == 0 {
            break;
        }

        for &byte in &read_buf[..n] {
            circular_buffer[total_written % num_bytes] = byte;
            total_written += 1;
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if total_written <= num_bytes {
        out.write_all(&circular_buffer[..total_written])?;
    } else {
        let start = total_written % num_bytes;
        out.write_all(&circular_buffer[start..])?;
        out.write_all(&circular_buffer[..start])?;
    }

    Ok(())
}

/// Output bytes starting from byte number `start_byte` (1-indexed).
/// Byte 1 means output the entire file, byte 2 means skip the first byte, etc.
pub fn tail_bytes_from_start<R: Read>(reader: R, start_byte: usize) -> io::Result<()> {
    let mut buf_reader = BufReader::new(reader);
    let skip = if start_byte > 0 { start_byte - 1 } else { 0 };

    // Skip the first (start_byte - 1) bytes
    io::copy(&mut buf_reader.by_ref().take(skip as u64), &mut io::sink())?;

    // Copy the rest to stdout
    let stdout = io::stdout();
    let mut out = stdout.lock();
    io::copy(&mut buf_reader, &mut out)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tail_bytes_from_start_logic() {
        // Test the skip logic
        let data = b"Hello, World!";
        let mut reader = io::Cursor::new(data);
        let skip = 7usize; // start_byte=8, skip 7
        io::copy(&mut reader.by_ref().take(skip as u64), &mut io::sink()).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(&result, b"World!");
    }

    #[test]
    fn test_circular_buffer_logic() {
        let data = b"Hello, World!";
        let num_bytes = 6;
        let mut circular_buffer = vec![0u8; num_bytes];
        let mut total_written = 0usize;

        for &byte in data.iter() {
            circular_buffer[total_written % num_bytes] = byte;
            total_written += 1;
        }

        // total_written=13, start=13%6=1
        // buffer = [!, W, o, r, l, d]
        // output: buf[1..] + buf[..1] = "World!"
        let mut result = Vec::new();
        let start = total_written % num_bytes;
        result.extend_from_slice(&circular_buffer[start..]);
        result.extend_from_slice(&circular_buffer[..start]);
        assert_eq!(std::str::from_utf8(&result).unwrap(), "World!");
    }
}
