use crate::error_message::ErrorMessage;
use crate::page::RawPage;
use tera::{Context, Tera};

#[derive(Clone)]
pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new(dir: &str) -> Result<Renderer, tera::Error> {
        let mut tera = Tera::new(dir)?;
        // Default error template used when an error occurs. Only add if an error template hasn't
        // been found int he directory.
        if !tera.get_template_names().any(|t| t.eq("error")) {
            tera.add_raw_template("error", include_str!("../templates/error.html")).unwrap();
        }
        // This template is added last to ensure that it is always available. If the error template
        // fails to render, this template will be used instead.
        tera.add_raw_template("error_fallback", include_str!("../templates/error_fallback.html")).unwrap();
        Ok(Renderer { tera })
    }

    pub fn render_page(&self, raw: &RawPage, template: &str) -> Result<String, tera::Error> {
        let mut context = Context::new();
        context.insert("title", &raw.metadata.title);
        context.insert("raw_cmark", &raw.markdown);

        let parser = pulldown_cmark::Parser::new(&raw.markdown);
        let mut rendered_cmark = String::new();
        pulldown_cmark::html::push_html(&mut rendered_cmark, parser);

        context.insert("rendered_cmark", &rendered_cmark);

        Ok(self.tera.render(template, &context)?)
    }

    /// Renders the error template with the provided title and error details. If the error template
    /// cannot be rendered, renders the fallback template.
    pub fn render_error(&self, error: &ErrorMessage) -> String {
        let mut context = Context::new();
        context.insert("title", &error.title);
        context.insert("details", &error.details);
        self.tera.render("error", &context).unwrap_or_else(Renderer::render_error_fallback)
    }

    fn render_error_fallback(err: tera::Error) -> String {
        let mut context = Context::new();
        context.insert("template_error", &err.to_string());
        Tera::default().render_str(include_str!("../templates/error_fallback.html"), &context)
            .expect("Failed to render error fallback template")
    }
}

impl Default for Renderer {
    fn default() -> Self {
        let mut tera = Tera::default();
        tera.add_raw_template("error",     r"
<html>
<head><title>{{ title }}</title></head>
<body>
    <h1>{{ title }}</h1>
    <p>{{ details }}</p>
</body>
</html>
").unwrap();
        Renderer { tera }
    }
}
