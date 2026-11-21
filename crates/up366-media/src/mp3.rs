//! MP3 probing.
//!
//! Only the container header is checked: an ID3v2 tag (`ID3`) or an MPEG frame
//! sync (`0xFF` followed by the top three bits set, i.e. `0xEx`). Anything
//! else is rejected before it reaches the upload path.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{MediaError, Result};

/// Metadata collected from a local audio file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFileInfo {
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of the whole file.
    pub sha256: String,
    /// Header-parsed MPEG duration estimate, when the file contains at least
    /// one parseable frame header. Decoder validation and encoder delay/padding
    /// are not included.
    pub duration_ms: Option<u64>,
}

/// Whether the first bytes of a file look like an MP3.
///
/// Accepts an ID3v2 tag or a raw MPEG audio frame sync. The input may be
/// shorter than a full header (for example a two-byte prefix of a frame).
pub fn is_mp3_header(bytes: &[u8]) -> bool {
    if bytes.len() >= 3 && &bytes[..3] == b"ID3" {
        return true;
    }
    bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0
}

/// Validate `path` as an MP3 and return its size, SHA-256 and a header-parsed
/// duration estimate.
///
/// Returns [`MediaError::NotMp3`] when the header does not match.
pub fn inspect_mp3(path: impl AsRef<Path>) -> Result<AudioFileInfo> {
    let path = path.as_ref();
    let mut file = File::open(path)?;

    let mut header = Vec::with_capacity(3);
    (&mut file).take(3).read_to_end(&mut header)?;
    if !is_mp3_header(&header) {
        return Err(MediaError::NotMp3(path.to_owned()));
    }

    // Hash the whole file, including the header bytes just consumed. Keeping
    // the bytes also lets us derive a header-parsed duration estimate without
    // trusting optional ID3/Xing metadata, which is often absent from files
    // exported by phones and video tools.
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut bytes = Vec::new();
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        bytes.extend_from_slice(&buf[..read]);
        size += read as u64;
    }
    let mut sha256 = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(sha256, "{byte:02x}").expect("writing to a String cannot fail");
    }

    Ok(AudioFileInfo {
        path: path.to_owned(),
        size,
        sha256,
        duration_ms: mp3_duration_ms(&bytes),
    })
}

/// Estimate an MPEG Layer III duration from parseable frame headers.
///
/// This intentionally does not depend on a decoder. It accepts MPEG-1,
/// MPEG-2 and MPEG-2.5 Layer III headers, including variable bitrate files, and
/// sums each frame's sample count at its own sample rate. Frame payloads are
/// not decoded, and encoder delay/padding are not accounted for. Invalid
/// trailing bytes are ignored after the last complete frame.
fn mp3_duration_ms(bytes: &[u8]) -> Option<u64> {
    let mut cursor = id3_end(bytes)?;
    let mut frames = 0u64;
    let mut duration_nanos = 0u128;

    while cursor + 4 <= bytes.len() {
        let Some(frame) = frame_at(bytes, cursor) else {
            cursor += 1;
            continue;
        };
        if cursor + frame.length > bytes.len() {
            break;
        }
        frames += 1;
        duration_nanos +=
            u128::from(frame.samples_per_frame) * 1_000_000_000 / u128::from(frame.sample_rate);
        cursor += frame.length;
    }

    (frames > 0).then_some((duration_nanos / 1_000_000) as u64)
}

fn id3_end(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 10 || &bytes[..3] != b"ID3" {
        return Some(0);
    }
    if bytes[6..10].iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    let size = bytes[6..10]
        .iter()
        .fold(0usize, |value, byte| (value << 7) | usize::from(*byte));
    let footer = usize::from(bytes[5] & 0x10 != 0) * 10;
    10usize.checked_add(size)?.checked_add(footer)
}

#[derive(Debug, Clone, Copy)]
struct MpegFrame {
    length: usize,
    sample_rate: u32,
    samples_per_frame: u16,
}

fn frame_at(bytes: &[u8], offset: usize) -> Option<MpegFrame> {
    let header = u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?);
    if bytes[offset] != 0xFF || (bytes[offset + 1] & 0xE0) != 0xE0 {
        return None;
    }

    let version = (header >> 19) & 0x3;
    let layer = (header >> 17) & 0x3;
    let bitrate_index = ((header >> 12) & 0xF) as usize;
    let sample_rate_index = ((header >> 10) & 0x3) as usize;
    let padding = ((header >> 9) & 1) as usize;
    if version == 1
        || layer != 1
        || bitrate_index == 0
        || bitrate_index == 15
        || sample_rate_index == 3
    {
        return None;
    }

    let bitrate_kbps = match version {
        3 => [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ][bitrate_index],
        2 | 0 => [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160][bitrate_index],
        _ => return None,
    } as usize;
    let base_rate = [44_100u32, 48_000, 32_000][sample_rate_index];
    let sample_rate = base_rate
        / match version {
            3 => 1,
            2 => 2,
            0 => 4,
            _ => return None,
        };
    let samples_per_frame = if version == 3 { 1_152 } else { 576 };
    let length = if version == 3 {
        144 * bitrate_kbps * 1_000 / sample_rate as usize + padding
    } else {
        72 * bitrate_kbps * 1_000 / sample_rate as usize + padding
    };
    (length >= 4).then_some(MpegFrame {
        length,
        sample_rate,
        samples_per_frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn header_accepts_id3_and_frame_sync() {
        assert!(is_mp3_header(b"ID3"));
        assert!(is_mp3_header(b"ID3\x04\x00\x00"));
        // MPEG-1 Layer III frame sync.
        assert!(is_mp3_header(&[0xFF, 0xFB, 0x90, 0x64]));
        assert!(is_mp3_header(&[0xFF, 0xE0]));
    }

    #[test]
    fn header_rejects_other_formats_and_short_input() {
        assert!(!is_mp3_header(b""));
        assert!(!is_mp3_header(&[0xFF]));
        assert!(!is_mp3_header(b"RIFF"));
        assert!(!is_mp3_header(b"ID"));
        // 0xFE has the frame-sync bit cleared.
        assert!(!is_mp3_header(&[0xFF, 0x1F]));
    }

    #[test]
    fn id3_file_is_accepted_with_size_and_hash() {
        let content = b"ID3hello";
        let file = write_temp(content);

        let info = inspect_mp3(file.path()).unwrap();
        assert_eq!(info.path, file.path());
        assert_eq!(info.size, content.len() as u64);
        // sha256("ID3hello")
        assert_eq!(
            info.sha256,
            "8fe0f72ff01d77c3069dcc3c64c4de672d51a999ce92e1be75bca3692fc1ff5e"
        );
        assert_eq!(info.duration_ms, None);
    }

    #[test]
    fn frame_sync_file_is_accepted() {
        let file = write_temp(&[0xFF, 0xFB, 0x90, 0x64, 0x00, 0x01]);
        let info = inspect_mp3(file.path()).unwrap();
        assert_eq!(info.size, 6);
        assert_eq!(info.sha256.len(), 64);
        assert_eq!(info.duration_ms, None);
    }

    #[test]
    fn duration_counts_mpeg_frames() {
        let frame = [0xFF, 0xFB, 0x90, 0x64];
        let mut bytes = Vec::new();
        for _ in 0..2 {
            bytes.extend_from_slice(&frame);
            bytes.resize(bytes.len() + 417 - frame.len(), 0);
        }
        let file = write_temp(&bytes);

        let info = inspect_mp3(file.path()).unwrap();

        assert_eq!(info.duration_ms, Some(52));
    }

    #[test]
    fn non_mp3_and_empty_files_are_rejected() {
        let wav = write_temp(b"RIFF\x24\x00\x00\x00WAVEfmt ");
        assert!(matches!(
            inspect_mp3(wav.path()),
            Err(MediaError::NotMp3(_))
        ));

        let empty = write_temp(b"");
        assert!(matches!(
            inspect_mp3(empty.path()),
            Err(MediaError::NotMp3(_))
        ));

        let missing = std::env::temp_dir().join("up366-no-such-file.mp3");
        assert!(matches!(inspect_mp3(&missing), Err(MediaError::Io(_))));
    }

    #[test]
    fn leading_junk_is_rejected() {
        let file = write_temp(b"junk-ID3");
        assert!(matches!(
            inspect_mp3(file.path()),
            Err(MediaError::NotMp3(_))
        ));
    }
}
