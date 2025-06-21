use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use rocket::{fs::NamedFile, http::Status, response::Redirect, Catcher, Request, Route, State};
use rocket_dyn_templates::Template;
use serde::Serialize;

use crate::config::Config;

#[cfg(not(debug_assertions))]
fn is_prod() -> bool {
    true
}

#[cfg(debug_assertions)]
fn is_prod() -> bool {
    false
}

#[derive(Debug)]
enum ContentResponse {
    Template(Template),
    Static(NamedFile)
}

impl<'r> rocket::response::Responder<'r, 'static> for ContentResponse {
    fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'static> {
        match self {
            ContentResponse::Template(template) => template.respond_to(req),
            ContentResponse::Static(file) => file.respond_to(req),
        }
    }
}

#[derive(Serialize)]
struct NavBarPage<'a> {
    title: &'a str,
    url: &'a str
}
#[derive(Serialize)]
struct PageContext<'a> {
    branding: &'a str,
    page_title: &'a str,
    page_url: &'a str,
    page_scripted: bool,
    template_name: &'a str,
    prod: bool,
    navigation: Vec<NavBarPage<'a>>,
}

#[derive(Serialize, Debug)]
pub struct TemplatePage {
    title: String,
    url: String,
    navigable: bool,
    scripted: bool
}

#[derive(Serialize)]
pub struct Templates {
    page_map: IndexMap<String, TemplatePage>
}

impl Templates {
    fn get_page(&self, path: &str) -> Option<&TemplatePage> {
        self.page_map.get(path)
    }
    fn get_navbar<'a>(&'a self) -> Vec<NavBarPage<'a>> {
        self.page_map.iter()
            .filter(|(_, page)| page.navigable)
            .map(|(url, page)| NavBarPage { url: url, title: &page.title }).collect()
    }
    pub fn load(path: &str) -> Result<Self, String> {
        let input_str = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to load {}: {}", path, e))?;

        let navigation_toml: toml::Table = toml::from_str(&input_str)
            .map_err(|e| format!("Failed to parse TOML input from {}: {}", path, e))?;

        let mut page_map = IndexMap::new();
        for (page, table) in navigation_toml {
            let title = table.get("title").and_then(|v| v.as_str())
                .ok_or_else(|| format!("'title' field for page {} is missing, or not a string.", page))?
                .to_string();
            let url = table.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| format!("'url' field for page {} is missing, or not a string.", page))?
                .to_string();
            let navigable = table.get("nav")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let scripted = table.get("script")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            page_map.insert(url.to_string(), TemplatePage { title, url, navigable, scripted });
        }
        Ok(Templates{page_map})
    }
}

#[rocket::get("/<path..>")]
async fn template_files(
    config: &State<Config>,
    templates: &State<Templates>,
    path: PathBuf
) -> Result<ContentResponse, Status> {
    if let Some(template_page) = templates.get_page(&path.display().to_string()) {
        info!("Matched template page {:?}", template_page);
        let template_name = template_page.url
            .strip_suffix(".html")
            .unwrap_or(&template_page.url)
            .to_string();
        let context = PageContext {
            branding: &config.branding,
            page_url: &template_page.url,
            page_title: &template_page.title,
            page_scripted: template_page.scripted,
            template_name: &template_name,
            prod: is_prod(),
            navigation: templates.get_navbar()
        };
        Ok(ContentResponse::Template(Template::render(template_name.clone(), context)))
    } else {
        let static_path = Path::new("static").join(&path);
        info!("No matching template for path {:?}, trying static path: {:?}", path, static_path);
        match NamedFile::open(static_path).await {
            Ok(file) => Ok(ContentResponse::Static(file)),
            Err(_) => Err(Status::NotFound)
        }
    }
}

#[rocket::get("/")]
fn index_redirect() -> Redirect {
    Redirect::to("/index.html")
}

#[catch(404)]
fn not_found(
    req: &Request<'_>
) -> Template {
    let config = req.rocket().state::<Config>().unwrap();
    let templates = req.rocket().state::<Templates>().unwrap();
    Template::render("404", PageContext {
        branding: &config.branding, page_title: "Not Found", page_url: "404.html", page_scripted: false, template_name: "404", prod: is_prod(), navigation: templates.get_navbar()
    })
}

pub(crate) fn routes() -> Vec<Route> {
    routes![crate::templates::index_redirect, template_files]
}

pub(crate) fn catchers() -> Vec<Catcher> {
    catchers![not_found]
}