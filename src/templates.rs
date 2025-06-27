use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use rocket::{fs::NamedFile, http::Status, request::{FromRequest, Outcome}, response::Redirect, Catcher, Request, Route, State};
use rocket_dyn_templates::Template;
use serde::Serialize;

use crate::config::Config;

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

#[derive(Debug, Serialize)]
pub struct NavBarPage<'a> {
    pub title: &'a str,
    pub url: &'a str
}
#[derive(Debug, Serialize)]
pub struct CommonPageContext<'a> {
    branding: &'a str,
    navigation: Vec<NavBarPage<'a>>,
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

        let templates = request.rocket().state::<Templates>();
        if templates.is_none() {
            return Outcome::Error((Status::InternalServerError, "missing Templates in request state".to_string()));
        }
        let templates = templates.unwrap();

        Outcome::Success(Self {
            branding: &config.branding,
            navigation: templates.get_navbar(),
            prod: cfg!(not(debug_assertions)),
            url: request.uri().path().as_str()
        })
    }
}
#[derive(Debug, Serialize)]
pub struct PageContext<'a> {
    pub title: &'a str,
    pub scripted: bool,
    pub template_name: &'a str,
}

#[derive(Serialize)]
struct NavigablePageContext<'a> {
    common: CommonPageContext<'a>,
    page: PageContext<'a>
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
    pub fn get_navbar<'a>(&'a self) -> Vec<NavBarPage<'a>> {
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
    common_context: CommonPageContext<'_>,
    templates: &State<Templates>,
    path: PathBuf
) -> Result<ContentResponse, Status> {
    let template_path = format!("/{}", path.display());
    if let Some(template_page) = templates.get_page(&template_path) {
        info!("Matched template page {:?}", template_page);
        let template_name = template_page.url
            .strip_suffix(".html")
            .and_then(|s| s.strip_prefix("/"))
            .unwrap_or(&template_page.url)
            .to_string();
        let page_context = PageContext {
            template_name: &template_name,
            title: &template_page.title,
            scripted: template_page.scripted,
        };
        Ok(ContentResponse::Template(Template::render(template_name.clone(), NavigablePageContext {
            common: common_context,
            page: page_context
        })))
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
async fn not_found(
    req: &Request<'_>
) -> Template {
    let common_context = CommonPageContext::from_request(req)
        .await
        .unwrap();
    Template::render("404", NavigablePageContext {
        common: common_context,
        page: PageContext {
            template_name: "404",
            title: "Not Found",
            scripted: false,
        }
    })
}

pub(crate) fn routes() -> Vec<Route> {
    routes![crate::templates::index_redirect, template_files]
}

pub(crate) fn catchers() -> Vec<Catcher> {
    catchers![not_found]
}