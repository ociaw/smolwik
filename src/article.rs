use crate::error_message::ErrorMessage;
use crate::filesystem;
use crate::filesystem::FileWriteError;
use crate::metadata::Metadata;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::path::Path;
use snafu::{ResultExt, Snafu};
use thiserror::Error;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter, Error, ErrorKind};

const MARKDOWN_SEPARATOR_LINUX: &'static str = "+++\n";
const MARKDOWN_SEPARATOR_WINDOWS: &'static str = "+++\r\n";

#[derive(Debug, Clone)]
pub struct RenderedArticle {
    pub status_code: StatusCode,
    pub html: Html<String>,
}

impl RenderedArticle {
    pub fn error(error: &ErrorMessage, html: String) -> RenderedArticle {
        RenderedArticle {
            status_code: error.status_code,
            html: Html(html),
        }
    }

    pub fn ok(html: String) -> RenderedArticle {
        RenderedArticle {
            status_code: StatusCode::OK,
            html: Html(html),
        }
    }

    pub fn internal_error(html: String) -> RenderedArticle {
        RenderedArticle {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            html: Html(html),
        }
    }
}

impl IntoResponse for RenderedArticle {
    fn into_response(self) -> Response {
        let mut response = self.html.into_response();
        *response.status_mut() = self.status_code;
        response
    }
}

#[derive(Debug, Clone, Default)]
pub struct RawArticle {
    pub metadata: Metadata,
    pub markdown: String,
}

impl RawArticle {
    pub async fn read_from_path(path: &Path) -> Result<RawArticle, ArticleReadError> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        RawArticle::from_reader(&mut reader).await
    }

    pub async fn from_reader<R>(mut reader: R) -> Result<RawArticle, ArticleReadError>
        where R: io::AsyncBufRead + Unpin
    {
        let mut str = String::new();
        reader.read_line(&mut str).await?;
        if !str.eq(MARKDOWN_SEPARATOR_LINUX) && !str.eq(MARKDOWN_SEPARATOR_WINDOWS) {
            eprintln!("Metadata start not found. Expected\n{}, found\n{}", MARKDOWN_SEPARATOR_LINUX, str);
            return Err(ArticleReadError::MissingMetadataStart);
        }
        drop(str);

        let mut metadata = String::new();
        let separator_len = loop {
            match reader.read_line(&mut metadata).await? {
                // If we read 0 bytes, that means we've reached the end of file without finding the
                // end marker.
                0 => {
                    eprintln!("Metadata end not found. Expected\n{}", MARKDOWN_SEPARATOR_LINUX);
                    return Err(ArticleReadError::MissingMetadataEnd)
                },
                4 if metadata.ends_with(MARKDOWN_SEPARATOR_LINUX) => break 4,
                5 if metadata.ends_with(MARKDOWN_SEPARATOR_WINDOWS) => break 5,
                _ => continue
            }
        };

        metadata.truncate(metadata.len().saturating_sub(separator_len));
        let metadata = toml::from_str(&metadata)?;

        let mut markdown = String::new();
        reader.read_to_string(&mut markdown).await?;

        Ok(RawArticle {
            metadata,
            markdown
        })
    }

    pub async fn write_to_path(&self, filepath: &Path, url_path: &str) -> Result<(), ArticleWriteError> {
        let mut file = filesystem::WritableFile::open(&filepath).await
            .or_else(|e| Err(ArticleWriteError::from_file_write_error(e, url_path.to_owned())))?;
        // Sure would be nice if we could use .context() here, but there's no way to make
        // UnhandlableIoSnafu public.
        self.write(&mut file.writer).await
            .with_context(|_| filesystem::UnhandlableIoSnafu { filepath })
            .with_context(|_| UnhandlableIoSnafu { path: url_path.to_owned() })?;
        file.close().await.with_context(|_| UnhandlableIoSnafu { path: url_path.to_owned() })?;
        Ok(())
    }

    pub async fn write(&self, writer: &mut BufWriter<File>) -> Result<(), io::Error> {
        writer.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;

        let toml = toml::to_string_pretty(&self.metadata).expect("Metadata serialization failed. This should never happen.");
        writer.write_all(toml.as_bytes()).await?;

        writer.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;
        writer.write_all(self.markdown.as_bytes()).await?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum ArticleReadError {
    /// Indicates that the requested article's path is invalid or doesn't exist.
    #[error("Article not found.")]
    NotFound,
    /// Indicates that there was an error reading the file.
    #[error("An error occurred reading the file at the provided path.")]
    IoError(io::Error),
    #[error("The start of the metadata section could not be found.")]
    MissingMetadataStart,
    #[error("The end of the metadata section could not be found.")]
    MissingMetadataEnd,
    #[error("The metadata is not valid TOML.")]
    InvalidMetadata(#[from] toml::de::Error),
}

impl From<io::Error> for ArticleReadError {
    fn from(value: Error) -> Self {
        match value.kind() {
            ErrorKind::NotFound | ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => ArticleReadError::NotFound,
            _ => ArticleReadError::IoError(value),
        }
    }
}

#[derive(Snafu, Debug)]
pub enum ArticleWriteError {
    #[snafu(display("Conflicting write in progress to {}: {}", path, source))]
    ConflictingWriteInProgress {
        source: FileWriteError,
        path: String,
    },
    /// Indicates that the article's path is invalid.
    #[snafu(display("Invalid file path {}: {}", path, source))]
    InvalidPath {
        source: FileWriteError,
        path: String,
    },
    /// Indicates that there was an error reading the file.
    #[snafu(display("Error when writing to {}: {}", path, source))]
    UnhandlableIoError{
        source: FileWriteError,
        path: String,
    }
}

impl ArticleWriteError {
    pub fn from_file_write_error(err: FileWriteError, path: String) -> ArticleWriteError {
        use crate::article::ArticleWriteError::*;

        match &err {
            FileWriteError::ConflictingWriteInProgress { source: _, filepath: _, .. } => ConflictingWriteInProgress { source: err, path },
            FileWriteError::UnhandlableIoError { source, filepath: _ } => match source.kind() {
                ErrorKind::NotFound | ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => InvalidPath { source: err, path },
                _ => UnhandlableIoError { source: err, path },
            }
        }
    }
}
