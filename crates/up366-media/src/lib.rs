//! Media pipeline.
//!
//! This crate is deliberately synchronous and network-free: it validates local
//! audio files and computes the object names used for OSS uploads. Uploading
//! itself belongs to the sync layer.

pub mod mp3;
pub mod object_name;

pub use mp3::{inspect_mp3, is_mp3_header, AudioFileInfo};
pub use object_name::object_name;

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    /// The file does not start with an MP3 header.
    #[error("not an mp3 file: {}", .0.display())]
    NotMp3(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MediaError>;
