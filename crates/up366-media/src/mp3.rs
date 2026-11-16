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

/// Validate `path` as an MP3 and return its size and SHA-256.
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

    // Hash the whole file, including the header bytes just consumed.
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
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
    }

    #[test]
    fn frame_sync_file_is_accepted() {
        let file = write_temp(&[0xFF, 0xFB, 0x90, 0x64, 0x00, 0x01]);
        let info = inspect_mp3(file.path()).unwrap();
        assert_eq!(info.size, 6);
        assert_eq!(info.sha256.len(), 64);
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
