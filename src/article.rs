use crate::error_message::ErrorMessage;
use crate::filesystem;
use crate::filesystem::FileWriteError;
use crate::metadata::Metadata;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::path::Path;
use snafu::{ResultExt, Snafu};
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

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
    pub async fn read_from_path(filepath: &Path, path: &str) -> Result<RawArticle, ArticleReadError> {
        let mut file = filesystem::ReadableFile::open(filepath).await.map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => ArticleReadError::NotFound { path: path.to_owned() },
            _ => ArticleReadError::IoError { source, path: path.to_owned() },
        })?;
        RawArticle::from_reader(&mut file.reader, path).await
    }

    pub async fn from_reader<R>(mut reader: R, path: &str) -> Result<RawArticle, ArticleReadError>
        where R: io::AsyncBufRead + Unpin
    {
        let mut str = String::new();
        reader.read_line(&mut str).await
            .with_context(|_| IoSnafu { path: path.to_owned() } )?;
        if !str.eq(MARKDOWN_SEPARATOR_LINUX) && !str.eq(MARKDOWN_SEPARATOR_WINDOWS) {
            eprintln!("Metadata start not found. Expected\n{}, found\n{}", MARKDOWN_SEPARATOR_LINUX, str);
            return Err(ArticleReadError::MissingMetadataStart { path: path.to_owned(), first_line: str })
        }
        drop(str);

        let mut metadata = String::new();
        let separator_len = loop {
            match reader.read_line(&mut metadata).await.with_context(|_| IoSnafu { path: path.to_owned() } )? {
                // If we read 0 bytes, that means we've reached the end of file without finding the
                // end marker.
                0 => {
                    eprintln!("Metadata end not found. Expected\n{}", MARKDOWN_SEPARATOR_LINUX);
                    return Err(ArticleReadError::MissingMetadataEnd { path: path.to_owned() })
                },
                4 if metadata.ends_with(MARKDOWN_SEPARATOR_LINUX) => break 4,
                5 if metadata.ends_with(MARKDOWN_SEPARATOR_WINDOWS) => break 5,
                _ => continue
            }
        };

        metadata.truncate(metadata.len().saturating_sub(separator_len));
        let metadata = toml::from_str(&metadata)
            .with_context(|_| InvalidMetadataSnafu { path: path.to_owned() } )?;

        let mut markdown = String::new();
        reader.read_to_string(&mut markdown).await
            .with_context(|_| IoSnafu { path: path.to_owned() } )?;

        Ok(RawArticle {
            metadata,
            markdown
        })
    }

    pub async fn write_to_path(&self, filepath: &Path, url_path: &str) -> Result<(), ArticleWriteError> {
        let mut file = filesystem::WritableFile::open(&filepath).await
            .or_else(|e| Err(ArticleWriteError::from_file_write_error(e, url_path.to_owned())))?;
        self.write(&mut file.writer).await
            .with_context(|_| filesystem::UnhandlableWriteSnafu { filepath })
            .with_context(|_| UnhandlableWriteSnafu { path: url_path.to_owned() })?;
        file.close().await.with_context(|_| UnhandlableWriteSnafu { path: url_path.to_owned() })?;
        Ok(())
    }

    pub async fn write<W>(&self, writer: &mut W) -> Result<(), io::Error>
        where W: io::AsyncWrite + Unpin {
        writer.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;

        let toml = toml::to_string_pretty(&self.metadata).expect("Metadata serialization failed. This should never happen.");
        writer.write_all(toml.as_bytes()).await?;

        writer.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;
        writer.write_all(self.markdown.as_bytes()).await?;
        Ok(())
    }
}

#[derive(Snafu, Debug)]
pub enum ArticleReadError {
    /// Indicates that the requested article's path is invalid or doesn't exist.
    #[snafu(display("No article found at {}", path))]
    NotFound { path: String },
    /// Indicates that there was an error reading the file.
    #[snafu(display("An error occurred reading the article at {}: {}", path, source))]
    IoError { source: io::Error, path: String },
    #[snafu(display("The start of the article metadata was not found in {}. Expected \n+++\nBut got\n{}", path, first_line))]
    MissingMetadataStart { path: String, first_line: String },
    #[snafu(display("The end of the article metadata was not found in {}: ", path))]
    MissingMetadataEnd  { path: String },
    #[snafu(display("Invalid non-TOML metadata found in article at {}: {}", path, source))]
    InvalidMetadata { source: toml::de::Error, path: String },
}

#[derive(Snafu, Debug)]
pub enum ArticleWriteError {
    #[snafu(display("Conflicting write in progress to {}.", path))]
    ConflictingWriteInProgress {
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
    UnhandlableWriteError {
        source: FileWriteError,
        path: String,
    }
}

impl ArticleWriteError {
    pub fn from_file_write_error(err: FileWriteError, path: String) -> ArticleWriteError {
        use crate::article::ArticleWriteError::*;
        use tokio::io::ErrorKind;

        match &err {
            FileWriteError::ConflictingWriteInProgress { filepath: _, .. } => ConflictingWriteInProgress { path },
            FileWriteError::UnhandlableWriteError { source, filepath: _ } => match source.kind() {
                ErrorKind::NotFound | ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => InvalidPath { source: err, path },
                _ => UnhandlableWriteError { source: err, path },
            }
        }
    }
}
