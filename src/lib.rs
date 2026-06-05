//! Core of the ADF transform.
//!
//! An `.adf` file is just an ordinary `.mp3` whose every byte has been XORed
//! with a single-byte key (`0x22` historically). Because `x ^ k ^ k == x`, the
//! XOR is its own inverse: the *same* operation converts both ways (MP3 → ADF
//! and ADF → MP3). This module therefore exposes one streaming routine,
//! [`xor_copy`], that both directions are built on.

use std::io::{self, Read, Write};

/// The historical ADF key. Every byte is XORed with this value.
pub const DEFAULT_KEY: u8 = 0x22; // 34

/// Working-buffer size. 64 KiB amortises read/write syscall overhead while
/// staying small enough to live comfortably in cache.
const BUF_SIZE: usize = 64 * 1024;

/// Stream every byte from `reader` to `writer`, XORing each with `key`.
///
/// After every chunk the `progress` callback is invoked with the running total
/// of bytes processed, allowing a caller to render a progress indicator without
/// this function knowing anything about the terminal. Returns the total number
/// of bytes transformed.
///
/// The inner XOR loop runs over a slice with a constant key, which the
/// optimiser readily auto-vectorises.
pub fn xor_copy<R, W, F>(
    mut reader: R,
    mut writer: W,
    key: u8,
    mut progress: F,
) -> io::Result<u64>
where
    R: Read,
    W: Write,
    F: FnMut(u64),
{
    let mut buf = vec![0u8; BUF_SIZE];
    let mut total: u64 = 0;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for byte in &mut buf[..n] {
            *byte ^= key;
        }
        writer.write_all(&buf[..n])?;
        total += n as u64;
        progress(total);
    }

    writer.flush()?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xor(data: &[u8], key: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let n = xor_copy(data, &mut out, key, |_| {}).unwrap();
        assert_eq!(n as usize, data.len());
        out
    }

    #[test]
    fn transform_matches_reference() {
        // Each byte XORed with the ADF key (34), as used by GTA Vice City
        let input = b"hello, mp3!";
        let expected: Vec<u8> = input.iter().map(|b| b ^ DEFAULT_KEY).collect();
        assert_eq!(xor(input, DEFAULT_KEY), expected);
    }

    #[test]
    fn round_trip_is_identity() {
        // Applying the transform twice yields the original bytes - this is why
        // a single operation converts in both directions.
        let input: Vec<u8> = (0..=255u8).cycle().take(200_000).collect();
        let once = xor(&input, DEFAULT_KEY);
        let twice = xor(&once, DEFAULT_KEY);
        assert_eq!(twice, input);
        assert_ne!(once, input);
    }

    #[test]
    fn empty_input_is_handled() {
        assert_eq!(xor(b"", DEFAULT_KEY), b"");
    }

    #[test]
    fn key_zero_is_a_no_op() {
        let input = b"unchanged";
        assert_eq!(xor(input, 0), input);
    }

    #[test]
    fn progress_reports_final_length() {
        let input = vec![0u8; BUF_SIZE * 2 + 7];
        let mut last = 0u64;
        let n = xor_copy(input.as_slice(), &mut Vec::new(), DEFAULT_KEY, |p| last = p).unwrap();
        assert_eq!(last, input.len() as u64);
        assert_eq!(n, input.len() as u64);
    }
}
