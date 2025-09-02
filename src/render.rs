use tera::{Context, Tera};
use crate::*;
use crate::auth::{Authorization, User};

#[derive(Clone)]
pub struct Renderer {
    config: Config,
    tera: Tera,
}

impl Renderer {
    pub fn new(config: Config) -> Result<Renderer, tera::Error> {
        let mut tera = Tera::new(&config.templates)?;
        // Default error template used when an error occurs. Only add if an error template hasn't
        // been found in the directory.
        if !tera.get_template_names().any(|t| t.eq("error")) {
            tera.add_raw_template("error", include_str!("../templates/error.tera")).unwrap();
        }
        // This template is added last to ensure that it is always available. If the error template
        // fails to render, this template will be used instead.
        tera.add_raw_template("error_fallback", include_str!("../templates/error_fallback.tera")).unwrap();
        Ok(Renderer { config, tera })
    }

    pub fn render_template(&self, user: &User, template: &str, title: &str) -> Result<String, tera::Error> {
        self.tera.render(template, &self.build_context(user, title))
    }

    pub fn render_template_with_context(&self, user: &User, template: &str, title: &str, context: Context) -> Result<String, tera::Error> {
        let mut ctx = self.build_context(user, title);
        ctx.extend(context);
        self.tera.render(template, &ctx)
    }

    /// Renders the error template with the provided title and error details. If the error template
    /// cannot be rendered, renders the fallback template.
    pub fn render_error(&self, user: &User, error: &ErrorResponse) -> String {
        let mut context = self.build_context(user, &error.title);
        context.insert("details", &error.details);
        self.tera.render("error", &context).unwrap_or_else(Renderer::render_error_fallback)
    }

    fn render_error_fallback(err: tera::Error) -> String {
        let mut context = Context::new();
        context.insert("template_error", &err.to_string());
        Tera::default().render_str(include_str!("../templates/error_fallback.tera"), &context)
            .expect("Failed to render error fallback template")
    }

    fn build_context(&self, user: &User, title: &str) -> Context {
        let mut context = context(title);
        context.insert("title", title);
        context.insert("auth_mode", self.config.auth_mode.variant_string());
        let authenticated = match user {
            User::Anonymous => false,
            User::SingleUser => true,
            User::Account(username) => {
                context.insert("username", username);
                true
            }
        };
        context.insert("can_create", &(user.check_authorization(&self.config.create_access) == Authorization::Authorized));
        context.insert("is_authenticated", &authenticated);
        context.insert("is_administrator", &(user.check_authorization(&self.config.administrator_access) == Authorization::Authorized));
        context
    }
}
