use crate::error_message::ErrorMessage;
use crate::metadata::Metadata;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Error, ErrorKind};

const METADATA_START: &'static str = "<!-- BEGIN METADATA\n";
const METADATA_END: &'static str = "END METADATA -->\n";

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

    pub fn write<W>(&self, mut writer: W) -> Result<(), PageWriteError>
        where W: std::io::Write
    {
        write!(writer, "<!-- BEGIN METADATA\n")?;
        serde_json::to_writer(&mut writer, &self.metadata).expect("Metadata serialization failed. This should never happen.");
        write!(writer, "\nEND METADATA -->\n")?;
        writer.write_all(self.html.0.as_bytes())?;
        Ok(())
    }
}

impl IntoResponse for RenderedPage {
    fn into_response(self) -> Response {
        let mut response = self.html.into_response();
        *response.status_mut() = self.status_code;
        response
    }
}

#[derive(Debug, Clone)]
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
        if !str.eq(METADATA_START) {
            eprintln!("{}", str);
            eprintln!("{}", METADATA_START);
            return Err(PageReadError::MissingMetadataStart);
        }
        drop(str);

        let mut metadata = String::new();
        while !metadata.ends_with(METADATA_END) {
            if reader.read_line(&mut metadata).await? == 0 {
                // If we read 0 bytes, that means we've reached the end of file without finding the
                // end marker.
                return Err(PageReadError::MissingMetadataEnd);
            }
        }

        metadata.truncate(metadata.len().saturating_sub(METADATA_END.len()));
        let metadata = serde_json::from_str(&metadata)?;

        let mut markdown = String::new();
        reader.read_to_string(&mut markdown).await?;

        Ok(RawPage {
            metadata,
            markdown
        })
    }

    pub async fn write_to_path(&self, path: &Path) -> Result<(), PageWriteError> {
        let file = File::create(path).await?;
        Ok(self.write(file).await?)
    }

    pub async fn write(&self, mut file: File) -> Result<(), PageWriteError> {
        file.write_all(METADATA_START.as_bytes()).await?;

        let mut file = file.into_std().await;
        serde_json::to_writer_pretty(&mut file, &self.metadata).expect("Metadata serialization failed. This should never happen.");
        let mut file = File::from_std(file);

        file.write_all(METADATA_END.as_bytes()).await?;
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
    #[error("The metadata is not valid JSON.")]
    InvalidMetadata(#[from] serde_json::Error),
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
