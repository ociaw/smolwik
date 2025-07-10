use tera::{Context, Error, Tera};
use tokio::fs;
use crate::metadata::Metadata;
use crate::page::{RawPage, RenderedPage};
use crate::PagePathset;

pub async fn page(pathset: &PagePathset, editable: bool) -> Result<RenderedPage, std::io::Error> {
    let raw_metadata = fs::read_to_string(&pathset.metadata).await?;
    let markdown = fs::read_to_string(&pathset.content).await?;

    let metadata: Metadata = match serde_json::from_str(&raw_metadata) {
        Ok(des) => des,
        Err(err) => return Ok(RenderedPage::internal_error(generic_error())),
    };

    let raw_page = RawPage { metadata, markdown };

    match render_page(raw_page, editable) {
        Ok(html) => Ok(RenderedPage::ok(html)),
        Err(err) => Ok(RenderedPage::internal_error(render_template_error(err))),
    }
}

fn render_page(raw: RawPage, editable: bool) -> Result<String, Error> {
    let tera = Tera::new("templates/**/*")?;

    let template = if editable { "page_edit.html" } else { "page.html" };

    let mut context = Context::new();
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

fn render_template_error(error: Error) -> String {
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

pub fn generic_error() -> String {
    r"
        <html>
        <head><title>An error occured when opening this page.</title></head>
        <body>
            <h1>An error occured when opening this page.</h1>
        </body>
        </html>
    ".to_owned()
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
