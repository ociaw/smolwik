use tera::{Context, Tera};
use crate::page::{PageReadError, PageWriteError, RawPage, RenderedPage};
use crate::{PagePathset};

pub async fn page(pathset: &PagePathset, editable: bool) -> Result<RenderedPage, RenderedPage> {
    let rendered = match RawPage::read_from_path(&pathset.content).await {
        Ok(raw) => match render_raw(&raw, editable) {
            Ok(html) => RenderedPage::ok(raw.metadata, html),
            Err(err) => RenderedPage::internal_error(render_template_error(err))
        },
        Err(err) => match err {
            // For transient IO errors, we don't want to save the response, so we return an error.
            PageReadError::IoError(io_err) => return Err(RenderedPage::internal_error(generic_error_message(&io_err.to_string()))),
            // These errors are not transient, and need to be fixed in some way. We render the
            // page with an error message and return that.
            PageReadError::NotFound => RenderedPage::not_found(not_found()),
            _ => RenderedPage::not_found(generic_error_message(&err.to_string())),
        }
    };

    Ok(rendered)
}

pub async fn page_write_error(error: PageWriteError) -> RenderedPage {
    match error {
        PageWriteError::InvalidPath => RenderedPage::bad_request(bad_request()),
        PageWriteError::IoError(err) => RenderedPage::internal_error(err.to_string()),
    }
}

fn render_raw(raw: &RawPage, editable: bool) -> Result<String, tera::Error> {
    let tera = Tera::new("templates/**/*")?;

    let template = if editable { "page_edit.html" } else { "page.html" };

    let mut context = Context::new();
    context.insert("title", &raw.metadata.title);
    if editable {
        context.insert("raw_cmark", &raw.markdown);
    }
    else {
        let parser = pulldown_cmark::Parser::new(&raw.markdown);

        let mut rendered_cmark = String::new();
        pulldown_cmark::html::push_html(&mut rendered_cmark, parser);
        context.insert("rendered_cmark", &rendered_cmark);
    }

    Ok(tera.render(template, &context)?)
}

fn render_template_error(error: tera::Error) -> String {
    let mut terra = Tera::default();

    let template = r"
        <html>
        <head><title>An error occured when rendering this template.</title></head>
        <body>
            <h1>An error occured when rendering this template:</h1>
            <p>{{ message }}</p>
        </body>
        </html>
    ";

    let mut context = Context::new();
    context.insert("message", &error.to_string());
    terra.render_str(template, &context).unwrap()
}

pub fn generic_error_message(message: &str) -> String {
    let mut terra = Tera::default();

    let template = r"
        <html>
        <head><title>An error occured when opening this page.</title></head>
        <body>
            <h1>An error occured when opening this page:</h1>
            <p>{{ message }}</p>
        </body>
        </html>
    ";

    let mut context = Context::new();
    context.insert("message", message);
    terra.render_str(template, &context).unwrap()
}

pub fn not_found() -> String {
    r"
        <html>
        <head><title>This page could not be found.</title></head>
        <body>
            <h1>This page could not be found.</h1>
        </body>
        </html>
    ".to_owned()
}


pub fn bad_request() -> String {
    r"
        <html>
        <head><title>Bad Request</title></head>
        <body>
            <h1>Bad request.</h1>
        </body>
        </html>
    ".to_owned()
}
