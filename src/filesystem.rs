use std::fs;
use snafu::prelude::*;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};

/// A file that has been opened for writing. Writes to a temp file and moves it to ensure resilience
/// to power-loss or program crashing.
#[derive(Debug)]
pub struct WritableFile {
    pub path: PathBuf,
    pub writer: BufWriter<File>,
    tmp_path: PathBuf,
}

impl WritableFile {
    pub async fn open(path: impl Into<PathBuf>) -> Result<WritableFile, FileWriteError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        }

        let tmp_path = path.with_added_extension("tmp");
        let file = File::create_new(&tmp_path)
            .await
            .or_else(|e| Err(FileWriteError::from_io_error_tmp(&path, e, &tmp_path)))?;
        let writer = BufWriter::new(file);
        Ok(WritableFile { path, writer, tmp_path })
    }

    pub async fn close(mut self) -> Result<(), FileWriteError> {
        let WritableFile {
            ref path,
            ref mut writer,
            ref tmp_path,
        } = self;

        writer
            .flush()
            .await
            .with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .with_context(|_| UnhandlableWriteSnafu { filepath: path.clone() })?;
        // Clear the tmp path so that we don't try to delete it on drop.
        self.tmp_path = PathBuf::new();
        Ok(())
    }
}

impl Drop for WritableFile {
    fn drop(&mut self) {
        // Try to ensure the temp file is deleted if the file wasn't closed properly.
        if self.tmp_path != PathBuf::new() {
            _ = fs::remove_file(&self.tmp_path)
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum FileWriteError {
    #[snafu(display("Conflicting write in progress to {}", filepath.display()))]
    ConflictingWriteInProgress { filepath: PathBuf, tmp_path: PathBuf },
    /// Indicates that an unhandlable error occurred when writing to the file.
    #[snafu(display("Error when writing to {}: {}", filepath.display(), source))]
    UnhandlableWriteError { source: io::Error, filepath: PathBuf },
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
#[derive(Debug)]
pub struct ReadableFile {
    pub reader: BufReader<File>,
}

impl<'a> ReadableFile {
    pub async fn open(path: &'a Path) -> Result<ReadableFile, io::Error> {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        Ok(ReadableFile { reader })
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;
    use snafu::{ResultExt, Whatever};
    use tokio::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use crate::filesystem::{ReadableFile, WritableFile};
    use crate::filesystem::FileWriteError::ConflictingWriteInProgress;
    use testdir::testdir;

    /// The happy path
    #[tokio::test]
    async fn write_read() -> Result<(), Whatever> {
        let dir = testdir!();
        let filepath = dir.join("file.txt");
        let mut file = WritableFile::open(&filepath).await.whatever_context("Couldn't open the file for write.")?;
        file.writer.write(b"Write/Read Test").await.whatever_context("Couldn't write to the file.")?;
        file.close().await.whatever_context("Failed to flush and closed file.")?;
        let mut file = ReadableFile::open(&filepath).await.whatever_context("Couldn't open the file for read.")?;
        let mut str = String::new();
        file.reader.read_to_string(&mut str).await.whatever_context("Couldn't read the file.")?;

        assert_eq!(str, "Write/Read Test");
        assert_matches!(fs::try_exists(&filepath).await, Ok(true));
        assert_matches!(fs::try_exists(&filepath.with_added_extension("tmp")).await, Ok(false));
        Ok(())
    }

    /// Tests that concurrent writes to the same file will return a [ConflictingWriteInProgress] error.
    #[tokio::test]
    async fn conflicting_write() -> Result<(), Whatever> {
        let dir = testdir!();
        let filepath = dir.join("file.txt");
        let mut file = WritableFile::open(&filepath).await.whatever_context("Couldn't open the file for write.")?;
        file.writer.write(b"Conflicting Write Test").await.whatever_context("Couldn't write to the file.")?;

        assert_matches!(WritableFile::open(&filepath).await, Err(ConflictingWriteInProgress { filepath: _, tmp_path: _ }));

        file.close().await.whatever_context("Couldn't close the file.")?;

        assert_matches!(fs::try_exists(&filepath).await, Ok(true));
        assert_matches!(fs::try_exists(&filepath.with_added_extension("tmp")).await, Ok(false));
        Ok(())
    }

    /// Tests that dropping a [WritableFile] before its closed doesn't affect the original file and
    /// that the temp file created is properly cleaned up.
    #[tokio::test]
    async fn cancelled_update() -> Result<(), Whatever> {
        let dir = testdir!();
        let filepath = dir.join("file.txt");
        let mut file = WritableFile::open(&filepath).await.whatever_context("Couldn't open the file for write.")?;
        file.writer.write(b"File Version 1").await.whatever_context("Couldn't write to the file.")?;
        file.close().await.whatever_context("Couldn't close the file.")?;

        let mut file = WritableFile::open(&filepath).await.whatever_context("Couldn't open the file for write, 2.")?;
        file.writer.write(b"File Version 2").await.whatever_context("Couldn't write to the file, 2.")?;
        file.writer.flush().await.whatever_context("Couldn't flush to the file, 2.")?;
        drop(file);

        let mut file = ReadableFile::open(&filepath).await.whatever_context("Couldn't open the file for read.")?;
        let mut str = String::new();
        file.reader.read_to_string(&mut str).await.whatever_context("Couldn't read the file.")?;

        assert_matches!(fs::try_exists(&filepath).await, Ok(true));
        assert_matches!(fs::try_exists(&filepath.with_added_extension("tmp")).await, Ok(false));
        assert_eq!(str, "File Version 1");
        Ok(())
    }

    /// Tests that dropping a [WritableFile] before its closed doesn't create a new file and
    /// that the temp file created is properly cleaned up.
    #[tokio::test]
    async fn cancelled_create() -> Result<(), Whatever> {
        let dir = testdir!();
        let filepath = dir.join("file.txt");
        let mut file = WritableFile::open(&filepath).await.whatever_context("Couldn't open the file for write.")?;
        file.writer.write(b"File Version 1").await.whatever_context("Couldn't write to the file.")?;
        drop(file);

        assert_matches!(fs::try_exists(&filepath).await, Ok(false));
        assert_matches!(fs::try_exists(&filepath.with_added_extension("tmp")).await, Ok(false));
        Ok(())
    }
}
