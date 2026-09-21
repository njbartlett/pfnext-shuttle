//! The blog: posts stored in Postgres, edited and read through the JSON API
//! by the single-page app, plus the images embedded in them ("blobs", kept
//! as Postgres large objects).
//!
//! Three groups of routes:
//! - `api_routes`, mounted under /api: posts and blob upload
//! - `blob_routes`, mounted under /blog: blob download at /blog/blobs/<id>,
//!   the URL existing post bodies embed
//! - `page_routes`, mounted at /: the SPA shell for /blog/posts/<title>.html
//!   with Open Graph tags for the post, so shared links get a preview
use std::path::PathBuf;

use async_stream::stream;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use futures::stream::{Stream, StreamExt};
use multer::{parse_boundary, Multipart};
use rocket::data::{Data, ToByteUnit};
use rocket::http::uri::Host;
use rocket::http::{ContentType, MediaType, Status};
use rocket::response::status::{Created, NoContent};
use rocket::response::{self, Responder};
use rocket::serde::json::Json;
use rocket::{Request, Response, Route, State};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::postgres::types::Oid;
use sqlx::{query, query_as, query_scalar, FromRow, PgPool, QueryBuilder};

use crate::apierror::ApiError;
use crate::common::to_internal_server_err;
use crate::config::Config;
use crate::loginsession::LoginSession;
use crate::templates::SPA_INDEX;
use crate::whereclause::{Operator, WhereClause};

const ROLE_EDITOR: &str = "editor";
const MAX_BLOB_SIZE_KIBIBYTES: usize = 512;
/// Length of the Open Graph description taken from the start of a post
const DESCRIPTION_CHARS: usize = 200;

pub fn api_routes() -> Vec<Route> {
    routes![list_posts, get_post, get_post_by_title, post_post, put_post, delete_post, post_blob]
}

pub fn blob_routes() -> Vec<Route> {
    routes![get_blob]
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
    published_at: Option<DateTime<FixedOffset>>
}
impl PostSummary {
    async fn query_list<Tz: TimeZone>(
        pool: &PgPool,
        published_after: Option<&DateTime<Tz>>,
        published_before: Option<&DateTime<Tz>>
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut qb = QueryBuilder::new("SELECT p.id, p.title, p.published_at,
                p.author_id, a.name AS author_name, a.email AS author_email, p.created_at,
                p.last_editor_id, l.name AS last_editor_name, l.email AS last_editor_email, p.last_edited_at
            FROM post AS p
            JOIN person AS a ON p.author_id = a.id
            JOIN person AS l ON p.last_editor_id = l.id");
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
    content: String
}
impl PostFull {
    const BASE_QUERY: &str = "SELECT p.id, p.title, p.published_at, p.content,
                    p.author_id, a.name AS author_name, a.email AS author_email, p.created_at,
                    p.last_editor_id, l.name AS last_editor_name, l.email AS last_editor_email, p.last_edited_at
                FROM post AS p
                JOIN person AS a ON p.author_id = a.id
                JOIN person AS l ON p.last_editor_id = l.id";
    async fn query_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        let mut query = Self::BASE_QUERY.to_string();
        query.push_str(" WHERE p.id = $1");
        query_as(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    async fn query_by_title(pool: &PgPool, title: &str) -> Result<Option<Self>, sqlx::Error> {
        let mut query = Self::BASE_QUERY.to_string();
        query.push_str(" WHERE p.title = $1");
        query_as(&query)
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
            Some(title) => Ok(Self::query_by_title(pool, &title).await?),
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
            published_at: published
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
/// unpublished drafts and posts scheduled for the future.
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

    let posts = PostSummary::query_list(pool.inner(), None, published_before.as_ref()).await
        .map_err(to_internal_server_err)?;
    Ok(Json(posts))
}

#[get("/posts/<id>")]
async fn get_post(pool: &State<PgPool>, id: i64) -> Result<Json<PostFull>, ApiError> {
    PostFull::query_by_id(pool, id).await
        .map_err(to_internal_server_err)?
        .map(Json)
        .ok_or_else(not_found)
}

/// The post with exactly this title (the SPA's post page is addressed by
/// title, as the legacy /blog/posts/<title>.html links were)
#[get("/posts/by-title/<title>")]
async fn get_post_by_title(pool: &State<PgPool>, title: &str) -> Result<Json<PostFull>, ApiError> {
    PostFull::query_by_title(pool, title).await
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
    let mut post = PostFull::query_by_id(pool, id).await
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

#[delete("/posts/<id>")]
async fn delete_post(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64
) -> Result<NoContent, ApiError> {
    require_editor(&login)?;
    let post = PostFull::query_by_id(pool, id).await
        .map_err(to_internal_server_err)?
        .ok_or_else(not_found)?;
    post.delete(pool)
        .await
        .map_err(to_internal_server_err)
        .map(|_| NoContent)
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
/// Blobs are stored once per content hash; the response is the URL to embed.
#[post("/blobs", data = "<data>")]
async fn post_blob(
    pool: &State<PgPool>,
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

    if let Some(mut field) = multipart.next_field().await.map_err(to_internal_server_err)? {
        let mut tx = pool.begin()
            .await
            .map_err(to_internal_server_err)?;

        // Create a new large object
        let loid: Oid = sqlx::query_scalar("SELECT lo_creat(-1)")
            .fetch_one(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;

        // Open the large object for writing
        let fd: i32 = sqlx::query_scalar("SELECT lo_open($1, 131072)") // 131072 = INV_WRITE
            .bind(loid)
            .fetch_one(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;
        let mut hasher = Sha256::new();

        // Stream the field's contents
        let mut size: usize = 0;
        while let Some(chunk) = field.chunk().await
                .map_err(|e| ApiError::new(Status::InternalServerError, e.to_string()))? {
            size += chunk.len();

            let bytes = chunk.to_vec();
            hasher.update(&bytes);
            query("SELECT lowrite($1, $2)")
                .bind(fd)
                .bind(&bytes[..chunk.len()])
                .execute(&mut *tx)
                .await
                .map_err(to_internal_server_err)?;
        }
        sqlx::query("SELECT lo_close($1)")
            .bind(fd)
            .execute(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;

        let mime_type = field.content_type()
            .map(|m| m.essence_str())
            .unwrap_or("application/octet-stream");

        let hash = hasher.finalize();
        let hash_hex = hex::encode(hash);
        info!("Stored blob {hash_hex} ({mime_type}, {size} bytes) for {}", login.email);

        // Insert metadata row
        let id: Option<String> = query_scalar("INSERT INTO blobs (id, mime_type, oid, size, created) VALUES ($1, $2, $3, $4, NOW()) ON CONFLICT DO NOTHING RETURNING id")
            .bind(&hash_hex)
            .bind(mime_type)
            .bind(loid)
            .bind(i64::try_from(size).map_err(to_internal_server_err)?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;

        // If the insert did not return a row, the record for this hash already exists. We need to delete the duplicate Large Object.
        if id.is_none() {
            query("SELECT lo_unlink($1)")
                .bind(loid)
                .execute(&mut *tx)
                .await
                .map_err(to_internal_server_err)?;
        }

        tx.commit()
            .await
            .map_err(to_internal_server_err)?;

        let location = format!("/blog/blobs/{hash_hex}");
        let result = PostBlobResult { location: location.clone() };
        Ok(Created::new(location).body(Json(result)))
    } else {
        Err(ApiError::new(Status::BadRequest, "missing multipart file".to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct MediaStream<S> {
    media_type: MediaType,
    stream: S
}

impl<S> MediaStream<S> {
    fn from_stream(media_type: MediaType, stream: S) -> Self {
        MediaStream { media_type, stream }
    }
}
impl<'r, S: futures::stream::Stream> Responder<'r, 'r> for MediaStream<S>
    where S: Send + 'r, S::Item: AsRef<[u8]> + Send + Unpin + 'r
{
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'r> {
        use rocket::response::stream::ReaderStream;

        Response::build()
            .header(ContentType(self.media_type))
            .streamed_body(ReaderStream::from(self.stream.map(std::io::Cursor::new)))
            .ok()
    }
}

#[derive(Debug, Deserialize, FromRow, Serialize)]
struct BlobMetadata {
    id: String,
    mime_type: String,
    size: i64,
    created: DateTime<FixedOffset>,
    oid: Oid
}
#[rocket::get("/blobs/<id>")]
async fn get_blob<'r>(
    pool: &'r State<PgPool>,
    id: &str
) -> Result<
        MediaStream<impl Stream<Item = Vec<u8>> + use<'r>>,
        ApiError
    > {
    let metadata: BlobMetadata = query_as("SELECT * FROM blobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| ApiError::new(Status::NotFound, format!("blob with id {id} not found")))?;
    let media_type = MediaType::parse_flexible(&metadata.mime_type)
        .ok_or_else(|| ApiError::new(Status::UnsupportedMediaType, metadata.mime_type))?;

    let mut tx = pool.begin()
        .await
        .map_err(to_internal_server_err)?;

    let lo_fd: i32 = sqlx::query_scalar("SELECT lo_open($1, 262144)") // 262144 = 0x40000 = INV_READ
        .bind(metadata.oid)
        .fetch_one(&mut *tx)
        .await
        .map_err(to_internal_server_err)?;

    let stream = stream! {
        loop {
            let chunk_result: Result<Option<Vec<u8>>, sqlx::Error> = query_scalar("SELECT loread($1, 2048)")
                .bind(lo_fd)
                .fetch_optional(&mut *tx)
                .await;
            if chunk_result.is_err() {
                error!("Failed to read next chunk from Large Object {:?}: {}", &metadata.oid, chunk_result.unwrap_err());
                break;
            }
            match chunk_result.unwrap() {
                Some(chunk) => {
                    if chunk.is_empty() {
                        break;
                    }
                    yield chunk;
                },
                None => {
                    break;
                }
            }
        }
        let _ = query("SELECT lo_close($1)")
            .bind(lo_fd)
            .execute(&mut *tx)
            .await
            .inspect_err(|e| warn!("Failed to close Large Object {:?}: {e}", metadata.oid));
        let _ = tx.commit()
            .await
            .inspect_err(|e| warn!("Failed to commit transaction: {e}"));
    };

    Ok(MediaStream::from_stream(media_type, stream))
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

    fn post(title: &str, content: &str) -> PostFull {
        let now = Utc::now().fixed_offset();
        PostFull {
            id: 7, title: title.to_string(), content: content.to_string(),
            author_id: 1, author_name: "admin".to_string(), author_email: "admin@example.com".to_string(), created_at: now,
            last_editor_id: 1, last_editor_name: "admin".to_string(), last_editor_email: "admin@example.com".to_string(), last_edited_at: now,
            published_at: Some(now)
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
    fn test_shell_without_head_is_unchanged() {
        assert_eq!(with_open_graph_tags("<p>x</p>", &post("t", ""), "b", "u"), "<p>x</p>");
    }
}
