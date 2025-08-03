use crate::*;
use crate::article::ArticleReadError;
use crate::auth::*;
use axum::extract::State;
use axum::response::Response;
use serde::Serialize;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:tree", get(tree_handler))
        .with_state(state)
}

#[derive(Serialize)]
pub struct DirectoryNode {
    pub file_path: PathBuf,
    pub url_path: String,
    pub name: String,
    pub files: Vec<FileNode>,
    pub directories: Vec<DirectoryNode>,
    pub has_index: bool,
}

#[derive(Serialize)]
pub struct FileNode {
    pub url_path: String,
    pub name: String,
}

impl DirectoryNode {
    fn new(path: impl Into<PathBuf>, url_path: impl Into<String>, stem: impl Into<String>) -> DirectoryNode {
        DirectoryNode {
            file_path: path.into(),
            url_path: url_path.into(),
            name: stem.into(),
            files: Vec::new(),
            directories: Vec::new(),
            has_index: false,
        }
    }
}

#[debug_handler]
async fn tree_handler(State(state): State<AppState>, user: User) -> Result<Response, Response> {
    check_access(&user, &state, &state.config.discovery_access)?;

    let mut root = DirectoryNode::new(&state.config.articles, "/", "");
    if let Err(err) = recurse_directory(&state.config.articles, &mut root).await {
        return Err(render_error(&state, &user, err.into()));
    }

    let mut context = Context::new();
    context.insert("discovery__tree_root", &root);
    Ok(render_template_with_context(&state, &user, "discovery.tree.tera", "Index", context)?)
}

async fn recurse_directory(article_root: &Path, mut parent: &mut DirectoryNode) -> Result<(), ArticleReadError> {
    // We populate and recurse separately - this ensures we only have one file handle at a time
    populate_directory(article_root, &mut parent).await?;
    for mut dir in &mut parent.directories {
        Box::pin(recurse_directory(article_root, &mut dir)).await?;
    }
    Ok(())
}

async fn populate_directory(article_root: &Path, dir: &mut DirectoryNode) -> Result<(), ArticleReadError> {
    for entry in std::fs::read_dir(&dir.file_path)? {
        let path = entry?.path();
        let url_path = path
            .strip_prefix(article_root).expect("All paths should be descendents of the article root.")
            .to_str().expect("All paths are expected to be valid UTF-8 strings.").to_owned();
        if let Some(stem) = path.file_stem() && let Some(stem) = stem.to_str() {
            let stem = stem.to_owned();
            if path.is_dir() {
                dir.directories.push(DirectoryNode::new(path, url_path, stem))
            } else if path.is_file()  {
                let article = RawArticle::read_from_path(&path).await?;
                if stem == "index" {
                    dir.has_index = true;
                    dir.name = article.metadata.title.clone();
                } else {
                    dir.files.push(FileNode { url_path, name: article.metadata.title.clone() })
                }
            }
        }
        dir.directories.sort_by(|first, second| first.name.cmp(&second.name));
        dir.files.sort_by(|first, second| first.name.cmp(&second.name));
    }
    Ok(())
}
