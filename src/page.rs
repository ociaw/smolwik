use crate::error_message::ErrorMessage;
use crate::metadata::Metadata;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Error, ErrorKind};

const MARKDOWN_SEPARATOR_LINUX: &'static str = "+++\n";
const MARKDOWN_SEPARATOR_WINDOWS: &'static str = "+++\r\n";

#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub metadata: Metadata,
    pub status_code: StatusCode,
    pub html: Html<String>,
}

impl RenderedPage {
    pub fn error(error: &ErrorMessage, html: String) -> RenderedPage {
        let metadata = match error.status_code {
            StatusCode::NOT_FOUND => Metadata::not_found(),
            StatusCode::BAD_REQUEST => Metadata::bad_request(),
            StatusCode::INTERNAL_SERVER_ERROR | _ => Metadata::internal_error(),
        };
        RenderedPage {
            metadata,
            status_code: error.status_code,
            html: Html(html),
        }
    }

    pub fn ok(metadata: Metadata, html: String) -> RenderedPage {
        RenderedPage {
            metadata,
            status_code: StatusCode::OK,
            html: Html(html),
        }
    }

    pub fn not_found(html: String) -> RenderedPage {
        RenderedPage {
            metadata: Metadata::not_found(),
            status_code: StatusCode::NOT_FOUND,
            html: Html(html),
        }
    }

    pub fn internal_error(html: String) -> RenderedPage {
        RenderedPage {
            metadata: Metadata::internal_error(),
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            html: Html(html),
        }
    }
}

impl IntoResponse for RenderedPage {
    fn into_response(self) -> Response {
        let mut response = self.html.into_response();
        *response.status_mut() = self.status_code;
        response
    }
}

#[derive(Debug, Clone, Default)]
pub struct RawPage {
    pub metadata: Metadata,
    pub markdown: String,
}

impl RawPage {
    pub async fn read_from_path(path: &Path) -> Result<RawPage, PageReadError> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        RawPage::from_reader(&mut reader).await
    }

    pub async fn from_reader<R>(mut reader: R) -> Result<RawPage, PageReadError>
        where R: io::AsyncBufRead + Unpin
    {
        let mut str = String::new();
        reader.read_line(&mut str).await?;
        if !str.eq(MARKDOWN_SEPARATOR_LINUX) && !str.eq(MARKDOWN_SEPARATOR_WINDOWS) {
            eprintln!("Metadata start not found. Expected\n{}, found\n{}", MARKDOWN_SEPARATOR_LINUX, str);
            return Err(PageReadError::MissingMetadataStart);
        }
        drop(str);

        let mut metadata = String::new();
        let separator_len = loop {
            match reader.read_line(&mut metadata).await? {
                // If we read 0 bytes, that means we've reached the end of file without finding the
                // end marker.
                0 => {
                    eprintln!("Metadata end not found. Expected\n{}", MARKDOWN_SEPARATOR_LINUX);
                    return Err(PageReadError::MissingMetadataEnd)
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

        Ok(RawPage {
            metadata,
            markdown
        })
    }

    pub async fn write_to_path(&self, path: &Path) -> Result<(), PageWriteError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file = File::create(path).await?;
        Ok(self.write(file).await?)
    }

    pub async fn write(&self, mut file: File) -> Result<(), PageWriteError> {

        file.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;

        let toml = toml::to_string_pretty(&self.metadata).expect("Metadata serialization failed. This should never happen.");
        file.write_all(toml.as_bytes()).await?;

        file.write_all(MARKDOWN_SEPARATOR_LINUX.as_bytes()).await?;
        file.write_all(self.markdown.as_bytes()).await?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum PageReadError {
    /// Indicates that the requested page's path is invalid or doesn't exist.
    #[error("Page not found.")]
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

impl From<io::Error> for PageReadError {
    fn from(value: Error) -> Self {
        match value.kind() {
            ErrorKind::NotFound | ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => PageReadError::NotFound,
            _ => PageReadError::IoError(value),
        }
    }
}

#[derive(Error, Debug)]
pub enum PageWriteError {
    /// Indicates that the requested page's path is invalid.
    #[error("Invalid path")]
    InvalidPath,
    /// Indicates that there was an error reading the file.
    #[error("An error occurred writing the file at the provided path.")]
    IoError(io::Error),
}

impl From<io::Error> for PageWriteError {
    fn from(value: Error) -> Self {
        match value.kind() {
            ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => PageWriteError::InvalidPath,
            _ => PageWriteError::IoError(value),
        }
    }
}
