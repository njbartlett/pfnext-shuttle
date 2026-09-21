//! Serving of the website: the Vite-built single-page app in web/dist, with
//! its shell returned for every page URL the client-side router owns.
use std::path::{Path, PathBuf};

use rocket::{fs::NamedFile, Route};

/// Output of `npm run build` (see web/README.md)
const WEB_DIST_DIR: &str = "web/dist";
/// The SPA shell, returned for every page URL the client-side router owns
pub const SPA_INDEX: &str = "web/dist/index.html";
/// Mount points whose unmatched paths must 404 rather than fall back to the SPA
const NON_SPA_PREFIXES: [&str; 1] = ["api"];

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

/// A file from the web build if there is one, otherwise the SPA shell for
/// page paths. `PathBuf` as a segments guard rejects `..`.
#[rocket::get("/<path..>", rank = 10)]
async fn static_or_spa(path: PathBuf) -> Option<NamedFile> {
    if let Some(file) = open_file(&Path::new(WEB_DIST_DIR).join(&path)).await {
        return Some(file);
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
        assert!(is_page_path(Path::new("blog/index.html")));
        assert!(is_page_path(Path::new("blog/unknown.html")));
    }

    #[test]
    fn test_assets_and_the_api_do_not() {
        assert!(!is_page_path(Path::new("img/missing.png")));
        assert!(!is_page_path(Path::new("assets/old.js")));
        assert!(!is_page_path(Path::new("api/unknown")));
    }
}
