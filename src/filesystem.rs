use std::path::{Path, PathBuf};
use snafu::prelude::*;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};

/// A file that has been opened for writing. Writes to a temp file and moves it to ensure resilience
/// to power-loss or program crashing.
pub struct WritableFile {
    pub path: PathBuf,
    pub writer: BufWriter<File>,
    tmp_path: PathBuf,
}

impl WritableFile {
    pub async fn open(path: impl Into<PathBuf>) -> Result<WritableFile, FileWriteError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        }

        let tmp_path = path.with_added_extension("tmp");
        let file = File::create_new(&tmp_path).await
            .or_else(|e| Err(FileWriteError::from_io_error_tmp(&path, e, &tmp_path)))?;
        let writer = BufWriter::new(file);
        Ok(WritableFile { path, writer, tmp_path })
    }

    pub async fn close(self) -> Result<(), FileWriteError> {
        let WritableFile { path, mut writer, tmp_path } = self;

        writer.flush().await.with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        drop(writer);
        tokio::fs::rename(&tmp_path, &path).await.with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        Ok(())
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum FileWriteError {
    #[snafu(display("Conflicting write in progress to {}", filepath.display()))]
    ConflictingWriteInProgress {
        filepath: PathBuf,
        tmp_path: PathBuf,
    },
    /// Indicates that an unhandlable error occurred when writing to the file.
    #[snafu(display("Error when writing to {}: {}", filepath.display(), source))]
    UnhandlableWriteError {
        source: io::Error,
        filepath: PathBuf
    },
}

impl FileWriteError {
    fn from_io_error_tmp(path: impl Into<PathBuf>, source: io::Error, tmp_path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let tmp_path = tmp_path.into();

        match source.kind() {
            io::ErrorKind::AlreadyExists => Self::ConflictingWriteInProgress {
                tmp_path,
                filepath: path,
            },
            _ => Self::UnhandlableWriteError { filepath: path, source },
        }
    }
}

/// A file that has been opened for reading.
pub struct ReadableFile<'a> {
    pub path: &'a Path,
    pub reader: BufReader<File>,
}

impl<'a> ReadableFile<'a> {
    pub async fn open(path: &'a Path) -> Result<ReadableFile<'a>, io::Error> {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        Ok(ReadableFile {
            path,
            reader
        })
    }
}
