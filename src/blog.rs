//! The blog: posts stored in Postgres, edited and read through the JSON API
//! by the single-page app. Images embedded in posts are uploaded to the S3
//! bucket and embedded by CDN URL.
//!
//! Two groups of routes:
//! - `api_routes`, mounted under /api: posts and image upload
//! - `page_routes`, mounted at /: the SPA shell for /blog/posts/<title>.html
//!   with Open Graph tags for the post, so shared links get a preview
use std::path::PathBuf;

use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use multer::{parse_boundary, Multipart};
use rocket::data::{Data, ToByteUnit};
use rocket::http::uri::Host;
use rocket::http::{ContentType, Status};
use rocket::response::status::{Created, NoContent};
use rocket::serde::json::Json;
use rocket::{Route, State};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, query_scalar, FromRow, PgPool, QueryBuilder};

use crate::apierror::ApiError;
use crate::common::to_internal_server_err;
use crate::config::Config;
use crate::loginsession::LoginSession;
use crate::s3::{self, S3Store};
use crate::templates::SPA_INDEX;
use crate::whereclause::{Operator, WhereClause};

const ROLE_EDITOR: &str = "editor";
const MAX_BLOB_SIZE_KIBIBYTES: usize = 512;
/// Where uploaded images go in the S3 bucket
const IMAGES_PREFIX: &str = "pfnext/blog/images";
/// Length of the Open Graph description taken from the start of a post
const DESCRIPTION_CHARS: usize = 200;

pub fn api_routes() -> Vec<Route> {
    routes![list_posts, get_post, get_post_by_title, post_post, put_post, put_post_publication, delete_post, put_post_read, post_blob]
}

pub fn page_routes() -> Vec<Route> {
    routes![post_page]
}

#[derive(FromRow, Debug, Deserialize, Serialize)]
struct PostSummary {
    id: i64,
    title: String,
    author_id: i64,
    author_name: String,
    author_email: String,
    created_at: DateTime<FixedOffset>,
    last_editor_id: i64,
    last_editor_name: String,
    last_editor_email: String,
    last_edited_at: DateTime<FixedOffset>,
    published_at: Option<DateTime<FixedOffset>>,
    /// When the logged-in reader first read the post; always None for anonymous readers
    read_at: Option<DateTime<FixedOffset>>,
    /// How many members have read the post; only included for editors
    read_count: Option<i64>
}
impl PostSummary {
    /// `reader_id` is the logged-in reader, whose read times are included
    async fn query_list<Tz: TimeZone>(
        pool: &PgPool,
        reader_id: Option<i64>,
        with_read_counts: bool,
        published_after: Option<&DateTime<Tz>>,
        published_before: Option<&DateTime<Tz>>
    ) -> Result<Vec<Self>, sqlx::Error> {
        let read_count = if with_read_counts {
            "(SELECT COUNT(*) FROM post_read AS c WHERE c.post_id = p.id)"
        } else {
            "NULL::bigint"
        };
        let mut qb = QueryBuilder::new(format!("SELECT p.id, p.title, p.published_at,
                p.author_id, a.name AS author_name, a.email AS author_email, p.created_at,
                p.last_editor_id, l.name AS last_editor_name, l.email AS last_editor_email, p.last_edited_at,
                r.read_at, {read_count} AS read_count
            FROM post AS p
            JOIN person AS a ON p.author_id = a.id
            JOIN person AS l ON p.last_editor_id = l.id
            LEFT JOIN post_read AS r ON r.post_id = p.id AND r.person_id = "));
        qb.push_bind(reader_id);
        let mut wc = WhereClause::init();

        if let Some(published_after) = published_after {
            wc.append_to(&mut qb, "p.published_at", Operator::GreaterThanOrEqual, published_after);
        }
        if let Some(published_before) = published_before {
            wc.append_to(&mut qb, "p.published_at", Operator::LessThanOrEqual, published_before);
        }
        qb.push(" ORDER BY p.created_at DESC");

        qb.build_query_as()
            .fetch_all(pool)
            .await
    }
}

#[derive(FromRow, Debug, Deserialize, Serialize)]
struct PostFull {
    id: i64,
    title: String,
    author_id: i64,
    author_name: String,
    author_email: String,
    created_at: DateTime<FixedOffset>,
    last_editor_id: i64,
    last_editor_name: String,
    last_editor_email: String,
    last_edited_at: DateTime<FixedOffset>,
    published_at: Option<DateTime<FixedOffset>>,
    /// When the logged-in reader first read the post; always None for anonymous readers
    read_at: Option<DateTime<FixedOffset>>,
    content: String
}
impl PostFull {
    /// $1 is the reader's person id, or NULL for an anonymous reader
    const BASE_QUERY: &str = "SELECT p.id, p.title, p.published_at, p.content,
                    p.author_id, a.name AS author_name, a.email AS author_email, p.created_at,
                    p.last_editor_id, l.name AS last_editor_name, l.email AS last_editor_email, p.last_edited_at,
                    r.read_at
                FROM post AS p
                JOIN person AS a ON p.author_id = a.id
                JOIN person AS l ON p.last_editor_id = l.id
                LEFT JOIN post_read AS r ON r.post_id = p.id AND r.person_id = $1";
    async fn query_by_id(pool: &PgPool, reader_id: Option<i64>, id: i64) -> Result<Option<Self>, sqlx::Error> {
        let mut query = Self::BASE_QUERY.to_string();
        query.push_str(" WHERE p.id = $2");
        query_as(&query)
            .bind(reader_id)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    async fn query_by_title(pool: &PgPool, reader_id: Option<i64>, title: &str) -> Result<Option<Self>, sqlx::Error> {
        let mut query = Self::BASE_QUERY.to_string();
        query.push_str(" WHERE p.title = $2");
        query_as(&query)
            .bind(reader_id)
            .bind(title)
            .fetch_optional(pool)
            .await
    }
    /// The post whose page is `location` (`<url-encoded title>.html`, the
    /// last segment of /blog/posts/... links)
    async fn from_location(pool: &PgPool, location: &str) -> Result<Option<Self>, sqlx::Error> {
        let title_opt = location.strip_suffix(".html")
            .and_then(|s| urlencoding::decode(s).ok());
        match title_opt {
            Some(title) => Ok(Self::query_by_title(pool, None, &title).await?),
            None => Ok(None)
        }
    }
    /// Path of the post's page, relative to /blog/posts/
    fn get_location(&self) -> String {
        let encoded_title = urlencoding::encode(&self.title);
        format!("{}.html", encoded_title)
    }
    fn page_path(&self) -> String {
        format!("/blog/posts/{}", self.get_location())
    }
    async fn create(
        pool: &PgPool,
        title: String,
        author_id: i64,
        author_name: String,
        author_email: String,
        published: Option<DateTime<FixedOffset>>,
        content: String
    ) -> Result<Self, sqlx::Error> {
        let now = Utc::now().fixed_offset();

        let id: i64 = query_scalar("INSERT INTO post (author_id, created_at, last_editor_id, last_edited_at, published_at, title, content) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id")
            .bind(author_id).bind(&now).bind(author_id).bind(&now).bind(&published).bind(&title).bind(&content)
            .fetch_one(pool)
            .await?;
        Ok(PostFull {
            id, title, content,
            author_id, author_name: author_name.clone(), author_email: author_email.clone(), created_at: now,
            last_editor_id: author_id, last_editor_name: author_name, last_editor_email: author_email, last_edited_at: now,
            published_at: published, read_at: None
        })
    }
    async fn save(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let result = query("UPDATE post
                SET author_id = $1, created_at = $2, last_editor_id = $3, last_edited_at = $4, published_at = $5, title = $6, content = $7
                WHERE id = $8
            ")
            .bind(self.author_id).bind(self.created_at).bind(self.last_editor_id).bind(self.last_edited_at).bind(self.published_at).bind(&self.title).bind(&self.content)
            .bind(self.id)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            Err(sqlx::Error::RowNotFound)
        } else {
            Ok(())
        }
    }
    async fn delete(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let result = query("DELETE FROM post WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            Err(sqlx::Error::RowNotFound)
        } else {
            Ok(())
        }
    }
}

/// Records that the person has read the post, keeping the time of the first
/// read. Fails with RowNotFound if there is no such post.
async fn record_read(pool: &PgPool, person_id: i64, post_id: i64) -> Result<(), sqlx::Error> {
    let post_exists: bool = query_scalar("WITH inserted AS (
                INSERT INTO post_read (person_id, post_id, read_at)
                SELECT $1, id, NOW() FROM post WHERE id = $2
                ON CONFLICT DO NOTHING
            )
            SELECT EXISTS (SELECT 1 FROM post WHERE id = $2)")
        .bind(person_id)
        .bind(post_id)
        .fetch_one(pool)
        .await?;
    if post_exists {
        Ok(())
    } else {
        Err(sqlx::Error::RowNotFound)
    }
}

fn require_editor(login: &LoginSession) -> Result<(), ApiError> {
    if login.is_editor() {
        Ok(())
    } else {
        Err(ApiError::new(Status::Forbidden, "admin or editor role required".to_string()))
    }
}

fn not_found() -> ApiError {
    ApiError::new(Status::NotFound, "not found".to_string())
}

/// Published posts, newest first. `all=true` (editors only) includes
/// unpublished drafts and posts scheduled for the future, and how many
/// members have read each post.
#[get("/posts?<all>")]
async fn list_posts(
    pool: &State<PgPool>,
    login: Option<LoginSession>,
    all: Option<bool>
) -> Result<Json<Vec<PostSummary>>, ApiError> {
    let all = all.unwrap_or(false);
    if all {
        require_editor(login.as_ref().ok_or_else(|| ApiError::new(Status::Unauthorized, "login required".to_string()))?)?;
    }
    let published_before = if all { None } else { Some(Utc::now().fixed_offset()) };

    let reader_id = login.as_ref().map(|l| l.uid);
    let posts = PostSummary::query_list(pool.inner(), reader_id, all, None, published_before.as_ref()).await
        .map_err(to_internal_server_err)?;
    Ok(Json(posts))
}

/// Readable by anyone; for a logged-in reader `read_at` says whether they
/// have read it before. Reading does not record a read: see `put_post_read`.
#[get("/posts/<id>")]
async fn get_post(pool: &State<PgPool>, login: Option<LoginSession>, id: i64) -> Result<Json<PostFull>, ApiError> {
    PostFull::query_by_id(pool, login.map(|l| l.uid), id).await
        .map_err(to_internal_server_err)?
        .map(Json)
        .ok_or_else(not_found)
}

/// The post with exactly this title (the SPA's post page is addressed by
/// title, as the legacy /blog/posts/<title>.html links were)
#[get("/posts/by-title/<title>")]
async fn get_post_by_title(pool: &State<PgPool>, login: Option<LoginSession>, title: &str) -> Result<Json<PostFull>, ApiError> {
    PostFull::query_by_title(pool, login.map(|l| l.uid), title).await
        .map_err(to_internal_server_err)?
        .map(Json)
        .ok_or_else(not_found)
}

#[derive(Debug, Deserialize, Serialize)]
struct SavePost {
    published: Option<DateTime<FixedOffset>>,
    title: String,
    content: String
}

#[post("/posts", data = "<update>")]
async fn post_post(
    pool: &State<PgPool>,
    login: LoginSession,
    update: Json<SavePost>
) -> Result<Created<Json<PostFull>>, ApiError> {
    require_editor(&login)?;
    let update = update.0;
    let post = PostFull::create(pool, update.title, login.uid, login.name, login.email, update.published, update.content)
        .await
        .map_err(to_internal_server_err)?;

    Ok(Created::new(post.page_path()).body(Json(post)))
}

/// Resolves to the post as saved; its title, and so its page path, may have changed
#[put("/posts/<id>", data = "<update>")]
async fn put_post(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64,
    update: Json<SavePost>
) -> Result<Json<PostFull>, ApiError> {
    require_editor(&login)?;
    let mut post = PostFull::query_by_id(pool, Some(login.uid), id).await
        .map_err(to_internal_server_err)?
        .ok_or_else(not_found)?;

    post.title = update.title.clone();
    post.content = update.content.clone();
    post.last_editor_id = login.uid;
    post.last_editor_name = login.name;
    post.last_editor_email = login.email;
    post.last_edited_at = Utc::now().fixed_offset();
    post.published_at = update.published;

    post.save(pool).await.map_err(to_internal_server_err)?;
    Ok(Json(post))
}

#[derive(Debug, Deserialize, Serialize)]
struct SavePublication {
    /// None unpublishes the post
    published: Option<DateTime<FixedOffset>>
}

/// Publishes or unpublishes a post without changing its content
#[put("/posts/<id>/publication", data = "<update>")]
async fn put_post_publication(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64,
    update: Json<SavePublication>
) -> Result<Json<PostFull>, ApiError> {
    require_editor(&login)?;
    let mut post = PostFull::query_by_id(pool, Some(login.uid), id).await
        .map_err(to_internal_server_err)?
        .ok_or_else(not_found)?;

    post.last_editor_id = login.uid;
    post.last_editor_name = login.name;
    post.last_editor_email = login.email;
    post.last_edited_at = Utc::now().fixed_offset();
    post.published_at = update.published;

    post.save(pool).await.map_err(to_internal_server_err)?;
    Ok(Json(post))
}

#[delete("/posts/<id>")]
async fn delete_post(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64
) -> Result<NoContent, ApiError> {
    require_editor(&login)?;
    let post = PostFull::query_by_id(pool, None, id).await
        .map_err(to_internal_server_err)?
        .ok_or_else(not_found)?;
    post.delete(pool)
        .await
        .map_err(to_internal_server_err)
        .map(|_| NoContent)
}

/// Marks the post as read by the logged-in user. Sent by the post page when
/// a member opens it, rather than recorded by the GETs, which the editor
/// also uses. Repeating it keeps the time of the first read.
#[put("/posts/<id>/read")]
async fn put_post_read(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64
) -> Result<NoContent, ApiError> {
    match record_read(pool, login.uid, id).await {
        Ok(()) => Ok(NoContent),
        Err(sqlx::Error::RowNotFound) => Err(not_found()),
        Err(e) => Err(to_internal_server_err(e))
    }
}

/// The SPA shell for a post's page, with the post's Open Graph tags added so
/// that links shared on social media and in chat get a title, description
/// and image. The page itself is rendered by the client-side router; an
/// unknown post gets the plain shell, on which the router shows "Not Found".
#[get("/blog/posts/<path..>")]
async fn post_page(
    pool: &State<PgPool>,
    config: &State<Config>,
    host: &Host<'_>,
    path: PathBuf
) -> Option<(ContentType, String)> {
    let shell = tokio::fs::read_to_string(SPA_INDEX).await.ok()?;
    let post = match PostFull::from_location(pool, &path.display().to_string()).await {
        Ok(post) => post,
        Err(e) => {
            error!("Failed to look up post for {}: {e}", path.display());
            None
        }
    };
    let html = match post {
        Some(post) => with_open_graph_tags(&shell, &post, &config.branding, &site_base_url(host)),
        None => shell
    };
    Some((ContentType::HTML, html))
}

/// The site's origin as this request saw it. Behind the production proxy the
/// scheme is https; only local development is plain http.
fn site_base_url(host: &Host<'_>) -> String {
    let host = host.to_string();
    let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") { "http" } else { "https" };
    format!("{scheme}://{host}")
}

/// Inserts the post's Open Graph tags before `</head>` and puts its title in
/// `<title>`. The shell is returned unchanged if it has no head.
fn with_open_graph_tags(shell: &str, post: &PostFull, branding: &str, base_url: &str) -> String {
    let Some(head_end) = shell.find("</head>") else {
        return shell.to_string();
    };
    let mut tags = String::new();
    let mut tag = |property: &str, content: &str| {
        tags.push_str(&format!("    <meta property=\"{property}\" content=\"{}\">\n", escape_attribute(content)));
    };
    tag("og:type", "article");
    tag("og:site_name", branding);
    tag("og:title", &post.title);
    tag("og:url", &format!("{base_url}{}", post.page_path()));
    let description = summarize(&post.content, DESCRIPTION_CHARS);
    if !description.is_empty() {
        tag("og:description", &description);
    }
    if let Some(image) = first_image_src(&post.content) {
        let absolute = if image.starts_with('/') { format!("{base_url}{image}") } else { image };
        tag("og:image", &absolute);
    }

    let mut html = String::with_capacity(shell.len() + tags.len());
    html.push_str(&shell[..head_end]);
    html.push_str(&tags);
    html.push_str(&shell[head_end..]);
    replace_title(&html, &format!("{} – {branding}", post.title))
}

fn replace_title(html: &str, title: &str) -> String {
    match (html.find("<title>"), html.find("</title>")) {
        (Some(start), Some(end)) if start < end => {
            format!("{}<title>{}{}", &html[..start], escape_text(title), &html[end..])
        }
        _ => html.to_string()
    }
}

/// The text of the post's HTML, whitespace collapsed, cut to about `max_chars`
fn summarize(html: &str, max_chars: usize) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut last_was_space = true;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                // Tag boundaries separate words ("<p>one</p><p>two</p>")
                if !last_was_space {
                    text.push(' ');
                    last_was_space = true;
                }
            }
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !last_was_space {
                    text.push(' ');
                    last_was_space = true;
                }
            }
            c => {
                text.push(c);
                last_was_space = false;
            }
        }
    }
    let text = decode_basic_entities(text.trim());
    if text.chars().count() <= max_chars {
        return text;
    }
    let cut: String = text.chars().take(max_chars).collect();
    // Back up to a word boundary
    let cut = match cut.rfind(' ') {
        Some(space) if space > max_chars / 2 => &cut[..space],
        _ => &cut
    };
    format!("{cut}…")
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The src of the first <img> in the post, if any
fn first_image_src(html: &str) -> Option<String> {
    let img = html.find("<img")?;
    let tag_end = html[img..].find('>').map(|i| img + i).unwrap_or(html.len());
    let tag = &html[img..tag_end];
    let src = tag.find("src=")? + 4;
    let quote = tag[src..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = src + 1;
    let value_end = tag[value_start..].find(quote)? + value_start;
    Some(decode_basic_entities(&tag[value_start..value_end]))
}

fn escape_attribute(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}

fn escape_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[derive(Deserialize, Serialize, Debug)]
struct PostBlobResult {
    location: String
}

/// Image upload from the post editor (a multipart form with one file field).
/// The image is stored publicly in the S3 bucket under a random name; the
/// response is its CDN URL, to embed in the post.
#[post("/blobs", data = "<data>")]
async fn post_blob(
    s3: &State<S3Store>,
    login: LoginSession,
    content_type: &ContentType,
    data: Data<'_>
) -> Result<Created<Json<PostBlobResult>>, ApiError> {
    use tokio_util::io::ReaderStream;

    require_editor(&login)?;

    let boundary = parse_boundary(&content_type.to_string())
        .map_err(|e| ApiError::new(Status::BadRequest, e.to_string()))?;

    // Turn rocket::Data into an AsyncRead
    let data_stream = data.open(MAX_BLOB_SIZE_KIBIBYTES.kibibytes());
    let stream = ReaderStream::new(data_stream);
    let mut multipart = Multipart::new(stream, boundary);

    let Some(field) = multipart.next_field().await.map_err(to_internal_server_err)? else {
        return Err(ApiError::new(Status::BadRequest, "missing multipart file".to_string()));
    };
    let mime_type = field.content_type()
        .map(|m| m.essence_str().to_string())
        .unwrap_or_default();
    let extension = image_extension(&mime_type)
        .ok_or_else(|| ApiError::new(Status::UnsupportedMediaType, format!("unsupported image type: {mime_type}")))?;
    let bytes = field.bytes().await
        .map_err(|e| ApiError::new(Status::BadRequest, e.to_string()))?;
    let size = bytes.len();

    let key = format!("{IMAGES_PREFIX}/{}.{extension}", s3::random_name());
    s3.upload(ByteStream::from(bytes), &mime_type, &key, true).await
        .map_err(to_internal_server_err)?;
    info!("Stored image {key} ({mime_type}, {size} bytes) for {}", login.email);

    let location = s3.public_url(&key);
    let result = PostBlobResult { location: location.clone() };
    Ok(Created::new(location).body(Json(result)))
}

/// The usual file extension for an image MIME type, or None if it is not an
/// image type that posts accept
fn image_extension(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/avif" => Some("avif"),
        "image/svg+xml" => Some("svg"),
        "image/bmp" => Some("bmp"),
        "image/tiff" => Some("tif"),
        _ => None
    }
}

impl LoginSession {
    fn is_editor(&self) -> bool {
        self.is_admin() || self.has_role(ROLE_EDITOR)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::testcommon::find_user_login_by_name;

    fn post(title: &str, content: &str) -> PostFull {
        let now = Utc::now().fixed_offset();
        PostFull {
            id: 7, title: title.to_string(), content: content.to_string(),
            author_id: 1, author_name: "admin".to_string(), author_email: "admin@example.com".to_string(), created_at: now,
            last_editor_id: 1, last_editor_name: "admin".to_string(), last_editor_email: "admin@example.com".to_string(), last_edited_at: now,
            published_at: Some(now), read_at: None
        }
    }

    const SHELL: &str = "<!DOCTYPE html><html><head><title>Another Level</title></head><body><div id=\"app\"></div></body></html>";

    #[test]
    fn test_open_graph_tags_describe_the_post() {
        let post = post("Park Run & Picnic", "<p>We met at <b>Oak Hill</b>.</p><p><img src=\"/blog/blobs/abc\" alt=\"\"> More text.</p>");
        let html = with_open_graph_tags(SHELL, &post, "Another Level", "https://example.org");

        assert!(html.contains("<title>Park Run &amp; Picnic – Another Level</title>"), "{html}");
        assert!(html.contains("<meta property=\"og:title\" content=\"Park Run &amp; Picnic\">"), "{html}");
        assert!(html.contains("<meta property=\"og:url\" content=\"https://example.org/blog/posts/Park%20Run%20%26%20Picnic.html\">"), "{html}");
        assert!(html.contains("<meta property=\"og:description\" content=\"We met at Oak Hill . More text.\">"), "{html}");
        assert!(html.contains("<meta property=\"og:image\" content=\"https://example.org/blog/blobs/abc\">"), "{html}");
        // The tags sit inside the head and the body is untouched
        assert!(html.find("og:image").unwrap() < html.find("</head>").unwrap());
        assert!(html.ends_with("<body><div id=\"app\"></div></body></html>"));
    }

    #[test]
    fn test_open_graph_tags_without_image_or_text() {
        let post = post("Empty", "<p></p>");
        let html = with_open_graph_tags(SHELL, &post, "Another Level", "http://localhost:8000");
        assert!(!html.contains("og:description"));
        assert!(!html.contains("og:image"));
        assert!(html.contains("content=\"http://localhost:8000/blog/posts/Empty.html\""));
    }

    #[test]
    fn test_summarize_cuts_at_a_word_boundary() {
        let text = summarize("<p>one two three four five</p>", 12);
        assert_eq!(text, "one two…");
        assert_eq!(summarize("short &amp; sweet", 100), "short & sweet");
    }

    #[test]
    fn test_first_image_src() {
        assert_eq!(first_image_src("<p>none</p>"), None);
        assert_eq!(first_image_src("<img class='x' src='a.png'><img src=\"b.png\">"), Some("a.png".to_string()));
        assert_eq!(first_image_src("<img alt=\"no source\">"), None);
    }

    #[test]
    fn test_image_extension() {
        assert_eq!(Some("jpg"), image_extension("image/jpeg"));
        assert_eq!(Some("png"), image_extension("image/png"));
        assert_eq!(Some("svg"), image_extension("image/svg+xml"));
        assert_eq!(None, image_extension("text/html"));
        assert_eq!(None, image_extension(""));
    }

    #[test]
    fn test_shell_without_head_is_unchanged() {
        assert_eq!(with_open_graph_tags("<p>x</p>", &post("t", ""), "b", "u"), "<p>x</p>");
    }

    async fn create_published_post(pool: &PgPool, title: &str) -> i64 {
        let admin = find_user_login_by_name(pool, "admin", "admin").await;
        let published = Some(Utc::now().fixed_offset() - chrono::Days::new(1));
        PostFull::create(pool, title.to_string(), admin.uid, admin.name, admin.email, published, "<p>x</p>".to_string())
            .await.unwrap()
            .id
    }

    async fn list_read_at(pool: &PgPool, login: Option<LoginSession>) -> Vec<(String, Option<DateTime<FixedOffset>>)> {
        list_posts(State::from(pool), login, None).await.unwrap().0
            .into_iter()
            .map(|p| (p.title, p.read_at))
            .collect()
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_reads_are_tracked_per_member(pool: PgPool) {
        let first = create_published_post(&pool, "First").await;
        create_published_post(&pool, "Second").await;
        let user1 = find_user_login_by_name(&pool, "user1", "member").await;
        let user2 = find_user_login_by_name(&pool, "user2", "member").await;

        put_post_read(State::from(&pool), user1.clone(), first).await.unwrap();

        let for_user1 = list_read_at(&pool, Some(user1.clone())).await;
        let read: Vec<&str> = for_user1.iter().filter(|(_, r)| r.is_some()).map(|(t, _)| t.as_str()).collect();
        assert_eq!(vec!["First"], read);
        assert!(list_read_at(&pool, Some(user2.clone())).await.iter().all(|(_, r)| r.is_none()));
        assert!(list_read_at(&pool, None).await.iter().all(|(_, r)| r.is_none()));

        assert!(get_post(State::from(&pool), Some(user1), first).await.unwrap().0.read_at.is_some());
        assert!(get_post(State::from(&pool), Some(user2), first).await.unwrap().0.read_at.is_none());
        // Anonymous readers can still read the post
        let anonymous = get_post_by_title(State::from(&pool), None, "First").await.unwrap().0;
        assert_eq!("<p>x</p>", anonymous.content);
        assert!(anonymous.read_at.is_none());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_rereading_keeps_the_first_read(pool: PgPool) {
        let id = create_published_post(&pool, "Post").await;
        let user1 = find_user_login_by_name(&pool, "user1", "member").await;

        put_post_read(State::from(&pool), user1.clone(), id).await.unwrap();
        let first_read = get_post(State::from(&pool), Some(user1.clone()), id).await.unwrap().0.read_at;
        put_post_read(State::from(&pool), user1.clone(), id).await.unwrap();
        let after_reread = get_post(State::from(&pool), Some(user1), id).await.unwrap().0.read_at;

        assert!(first_read.is_some());
        assert_eq!(first_read, after_reread);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_reading_a_missing_post_is_not_found(pool: PgPool) {
        let user1 = find_user_login_by_name(&pool, "user1", "member").await;
        let err = put_post_read(State::from(&pool), user1, 999).await.unwrap_err();
        assert_eq!(Status::NotFound, err.0);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_deleting_a_post_deletes_its_reads(pool: PgPool) {
        let id = create_published_post(&pool, "Post").await;
        let user1 = find_user_login_by_name(&pool, "user1", "member").await;
        let admin = find_user_login_by_name(&pool, "admin", "admin").await;
        put_post_read(State::from(&pool), user1, id).await.unwrap();

        delete_post(State::from(&pool), admin, id).await.unwrap();

        let reads: i64 = query_scalar("SELECT COUNT(*) FROM post_read").fetch_one(&pool).await.unwrap();
        assert_eq!(0, reads);
    }

    #[test]
    fn test_editors_are_not_admins() {
        let login = |roles: &str| LoginSession { roles: crate::loginsession::Roles::parse(roles), ..login_without_roles() };
        assert!(login("editor").is_editor());
        assert!(!login("editor").is_admin());
        assert!(login("admin").is_editor());
        assert!(!login("member").is_editor());
        assert!(!login("trainer").is_editor());
    }

    fn login_without_roles() -> LoginSession {
        LoginSession {
            sessionid: "xxx".to_string(), uid: 0, name: "x".to_string(), email: "x@example.com".to_string(),
            roles: crate::loginsession::Roles::parse(""), status: None, loggedin: None, loggedin_from: None,
            expiry: Utc::now().fixed_offset()
        }
    }

    fn save_post(title: &str, content: &str, published: Option<DateTime<FixedOffset>>) -> Json<SavePost> {
        Json(SavePost { title: title.to_string(), content: content.to_string(), published })
    }

    async fn post_titled(pool: &PgPool, title: &str) -> Option<PostFull> {
        PostFull::query_by_title(pool, None, title).await.unwrap()
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_editor_manages_posts(pool: PgPool) {
        let editor = find_user_login_by_name(&pool, "user1", "member,editor").await;

        // Create a draft, which only editors can list
        post_post(State::from(&pool), editor.clone(), save_post("Draft", "<p>one</p>", None)).await.unwrap();
        let draft = post_titled(&pool, "Draft").await.unwrap();
        assert_eq!(editor.uid, draft.author_id);
        let all = list_posts(State::from(&pool), Some(editor.clone()), Some(true)).await.unwrap().0;
        assert_eq!(vec!["Draft"], all.iter().map(|p| p.title.as_str()).collect::<Vec<_>>());

        // Edit every field, on its own post and on the admin's
        let published = Some(Utc::now().fixed_offset() - chrono::Days::new(1));
        put_post(State::from(&pool), editor.clone(), draft.id, save_post("Published", "<p>two</p>", published)).await.unwrap();
        let edited = post_titled(&pool, "Published").await.unwrap();
        assert_eq!(("<p>two</p>", published.map(|p| p.timestamp())), (edited.content.as_str(), edited.published_at.map(|p| p.timestamp())));

        let admins = create_published_post(&pool, "Admin's post").await;
        put_post(State::from(&pool), editor.clone(), admins, save_post("Admin's post, edited", "<p>three</p>", None)).await.unwrap();
        let edited = post_titled(&pool, "Admin's post, edited").await.unwrap();
        assert_eq!((editor.uid, None), (edited.last_editor_id, edited.published_at));

        // Delete both
        delete_post(State::from(&pool), editor.clone(), draft.id).await.unwrap();
        delete_post(State::from(&pool), editor.clone(), admins).await.unwrap();
        assert!(list_posts(State::from(&pool), Some(editor), Some(true)).await.unwrap().0.is_empty());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_read_counts_are_listed_for_editors_only(pool: PgPool) {
        let id = create_published_post(&pool, "Post").await;
        let admin = find_user_login_by_name(&pool, "admin", "admin").await;
        for name in ["user1", "user2"] {
            let reader = find_user_login_by_name(&pool, name, "member").await;
            put_post_read(State::from(&pool), reader, id).await.unwrap();
        }

        let all = list_posts(State::from(&pool), Some(admin.clone()), Some(true)).await.unwrap().0;
        assert_eq!(Some(2), all[0].read_count);
        let published = list_posts(State::from(&pool), Some(admin), None).await.unwrap().0;
        assert_eq!(None, published[0].read_count);
        let anonymous = list_posts(State::from(&pool), None, None).await.unwrap().0;
        assert_eq!(None, anonymous[0].read_count);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_editor_publishes_and_unpublishes(pool: PgPool) {
        let id = create_published_post(&pool, "Post").await;
        let editor = find_user_login_by_name(&pool, "user1", "member,editor").await;
        let publication = |published| Json(SavePublication { published });

        let unpublished = put_post_publication(State::from(&pool), editor.clone(), id, publication(None)).await.unwrap().0;
        assert_eq!((None, editor.uid), (unpublished.published_at, unpublished.last_editor_id));
        assert!(list_posts(State::from(&pool), None, None).await.unwrap().0.is_empty());

        let now = Utc::now().fixed_offset();
        put_post_publication(State::from(&pool), editor.clone(), id, publication(Some(now))).await.unwrap();
        let republished = post_titled(&pool, "Post").await.unwrap();
        assert_eq!(Some(now.timestamp()), republished.published_at.map(|p| p.timestamp()));
        assert_eq!("<p>x</p>", republished.content);

        let missing = put_post_publication(State::from(&pool), editor, 999, publication(None)).await;
        assert!(matches!(missing, Err(ApiError(status, _)) if status == Status::NotFound));
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_members_cannot_manage_posts(pool: PgPool) {
        let id = create_published_post(&pool, "Post").await;
        for roles in ["member", "trainer"] {
            let login = find_user_login_by_name(&pool, "user1", roles).await;
            let forbidden = |result: Result<_, ApiError>| matches!(result, Err(ApiError(status, _)) if status == Status::Forbidden);

            assert!(forbidden(post_post(State::from(&pool), login.clone(), save_post("New", "", None)).await.map(|_| ())), "{roles}");
            assert!(forbidden(put_post(State::from(&pool), login.clone(), id, save_post("Changed", "", None)).await.map(|_| ())), "{roles}");
            assert!(forbidden(put_post_publication(State::from(&pool), login.clone(), id, Json(SavePublication { published: None })).await.map(|_| ())), "{roles}");
            assert!(forbidden(delete_post(State::from(&pool), login.clone(), id).await.map(|_| ())), "{roles}");
            assert!(forbidden(list_posts(State::from(&pool), Some(login), Some(true)).await.map(|_| ())), "{roles}");
        }
        let anonymous = list_posts(State::from(&pool), None, Some(true)).await.map(|_| ());
        assert!(matches!(anonymous, Err(ApiError(status, _)) if status == Status::Unauthorized));
        assert_eq!("Post", post_titled(&pool, "Post").await.unwrap().title);
    }
}
