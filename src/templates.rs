//! Serving of the website: the Vite-built single-page app in web/dist, the
//! legacy assets in static/ that the Tera-rendered blog pages still load,
//! and the Tera page context those blog pages are rendered with.
use std::path::{Path, PathBuf};

use rocket::{fs::NamedFile, http::Status, request::{FromRequest, Outcome}, Request, Route, State};
use serde::Serialize;

use crate::config::Config;

/// Output of `npm run build` (see web/README.md)
const WEB_DIST_DIR: &str = "web/dist";
/// Bootstrap, Vue, library.js, TinyMCE and styles used by the Tera blog pages
const STATIC_DIR: &str = "static";
/// The SPA shell, returned for every page URL the client-side router owns
const SPA_INDEX: &str = "web/dist/index.html";
/// Mount points whose unmatched paths must 404 rather than fall back to the SPA
const NON_SPA_PREFIXES: [&str; 2] = ["api", "blog"];

/// Tera context shared by the remaining server-rendered (blog) pages
#[derive(Debug, Serialize)]
pub struct CommonPageContext<'a> {
    branding: &'a str,
    prod: bool,
    url: &'a str,
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for CommonPageContext<'r> {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = request.rocket().state::<Config>();
        if config.is_none() {
            return Outcome::Error((Status::InternalServerError, "missing Config in request state".to_string()));
        }
        let config = config.unwrap();

        Outcome::Success(Self {
            branding: &config.branding,
            prod: cfg!(not(debug_assertions)),
            url: request.uri().path().as_str()
        })
    }
}
#[derive(Debug, Serialize)]
pub struct PageContext<'a> {
    pub title: &'a str,
    /// Names the Tera template and its static/js/<template_name>.js script
    pub template_name: &'a str,
}

/// True for paths the client-side router should handle when no file matches:
/// page URLs (with or without a ".html" suffix) but not missing assets
fn is_page_path(path: &Path) -> bool {
    if NON_SPA_PREFIXES.iter().any(|prefix| path.starts_with(prefix)) {
        return false;
    }
    match path.extension() {
        None => true,
        Some(extension) => extension == "html"
    }
}

async fn open_file(path: &Path) -> Option<NamedFile> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => NamedFile::open(path).await.ok(),
        _ => None
    }
}

async fn spa_index() -> Option<NamedFile> {
    open_file(Path::new(SPA_INDEX)).await
}

#[rocket::get("/")]
async fn root() -> Option<NamedFile> {
    spa_index().await
}

/// Static files first (the web build, then the legacy assets), otherwise the
/// SPA shell for page paths. `PathBuf` as a segments guard rejects `..`.
#[rocket::get("/<path..>", rank = 10)]
async fn static_or_spa(path: PathBuf, _config: &State<Config>) -> Option<NamedFile> {
    for dir in [WEB_DIST_DIR, STATIC_DIR] {
        if let Some(file) = open_file(&Path::new(dir).join(&path)).await {
            return Some(file);
        }
    }
    if is_page_path(&path) {
        info!("No file for {:?}, serving the SPA shell", path);
        spa_index().await
    } else {
        None
    }
}

pub(crate) fn routes() -> Vec<Route> {
    routes![root, static_or_spa]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_page_path;

    #[test]
    fn test_page_paths_fall_back_to_spa() {
        assert!(is_page_path(Path::new("sessions.html")));
        assert!(is_page_path(Path::new("some/deep/page")));
        assert!(is_page_path(Path::new("nonexistent.html")));
    }

    #[test]
    fn test_assets_and_other_mounts_do_not() {
        assert!(!is_page_path(Path::new("img/missing.png")));
        assert!(!is_page_path(Path::new("js/pages/old.js")));
        assert!(!is_page_path(Path::new("api/unknown")));
        assert!(!is_page_path(Path::new("blog/unknown.html")));
    }
}
