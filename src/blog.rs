use std::path::PathBuf;

use async_stream::stream;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use futures::stream::{Stream, StreamExt};
use rocket::form::FromFormField;
use sha2::{Digest, Sha256};
use multer::{parse_boundary, Multipart};
use rocket::data::{Data, ToByteUnit};
use rocket::http::{ContentType, MediaType, Status};
use rocket::serde::json::Json;
use rocket::{Request, Response, Route, State};
use rocket::response::{self, Responder};
use rocket::response::status::{Created, Custom, NoContent};
use rocket_dyn_templates::{context, Template};
use serde::{Deserialize, Serialize};
use sqlx::postgres::types::Oid;
use sqlx::{query, query_as, query_scalar, FromRow, PgPool, QueryBuilder};

use crate::common::to_internal_server_err;
use crate::loginsession::LoginSession;
use crate::templates::{CommonPageContext, PageContext};
use crate::whereclause::{Operator, WhereClause};

const ROLE_EDITOR: &str = "editor";
const INDEX_TEMPLATE: &str = "blog_index";
const POST_TEMPLATE: &str = "blog_post";
const EDIT_POST_TEMPLATE: &str = "edit_post";
const MAX_BLOB_SIZE_KIBIBYTES: usize = 512;

pub fn routes() -> Vec<Route> {
    routes![
        // Template-based HTML pages
        get_index, get_post, get_post_editor,

        // API for creating, saving & deleting posts
        put_post, post_post, delete_post,

        // API for managing blobs
        post_blob, get_blob,
    ]
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
    async fn query_by_title(pool: &PgPool,title: &str) -> Result<Option<Self>, sqlx::Error> {
        let mut query = Self::BASE_QUERY.to_string();
        query.push_str(" WHERE p.title = $1");
        query_as(&query)
            .bind(title)
            .fetch_optional(pool)
            .await
    }
    async fn from_location(pool: &PgPool, location: &str) -> Result<Option<Self>, sqlx::Error> {
        let title_opt = location.strip_suffix(".html")
            .and_then(|s| urlencoding::decode(s).ok());
        info!("Finding post for location {location} => {title_opt:?}");
        match title_opt {
            Some(title) => Ok(Self::query_by_title(pool, &title).await?),
            None => Ok(None)
        }
    }
    fn get_location(&self) -> String {
        let encoded_title = urlencoding::encode(&self.title);
        format!("{}.html", encoded_title)
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
            author_id: author_id, author_name: author_name.clone(), author_email: author_email.clone(), created_at: now,
            last_editor_id: author_id, last_editor_name: author_name, last_editor_email: author_email, last_edited_at: now,
            published_at: published
        })
    }
    async fn save(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let result = query("UPDATE post
                SET author_id = $1, created_at = $2, last_editor_id = $3, last_edited_at = $4, published_at = $5, title = $6, content = $7
                WHERE id = $8
            ")
            .bind(&self.author_id).bind(&self.created_at).bind(&self.last_editor_id).bind(&self.last_edited_at).bind(&self.published_at).bind(&self.title).bind(&self.content)
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
            .bind(&self.id)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            Err(sqlx::Error::RowNotFound)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
enum IndexMode {
    Normal, AllPosts
}
impl<'r> FromFormField<'r> for IndexMode {
    fn from_value(field: rocket::form::ValueField<'r>) -> rocket::form::Result<'r,Self> {
        match field.value.to_lowercase().as_str() {
            "allposts" => Ok(Self::AllPosts),
            _ => Ok(Self::Normal)
        }
    }
    fn default() -> Option<Self> {
        Some(Self::Normal)
    }
}


#[get("/index.html?<mode>")]
async fn get_index(
    common_context: CommonPageContext<'_>,
    pool: &State<PgPool>,
    login: Option<LoginSession>,
    mode: IndexMode
) -> Result<Template, Custom<String>> {
    if mode == IndexMode::AllPosts && !login.map(|l| l.is_editor()).unwrap_or(false) {
        return Err(Custom(Status::Forbidden, "admin or editor role required".to_string()));
    }

    let published_before = match mode {
        IndexMode::Normal => Some(Utc::now().fixed_offset()),
        IndexMode::AllPosts => None
    };

    let posts = PostSummary::query_list(pool.inner(), None, published_before.as_ref()).await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    let context = context!{
        common: common_context,
        page: PageContext {
            title: "Posts",
            scripted: true,
            template_name: INDEX_TEMPLATE,
        },
        posts,
        index_mode: mode
    };
    Ok(Template::render(INDEX_TEMPLATE, context))
}

#[get("/posts/<path..>")]
async fn get_post(
    common_context: CommonPageContext<'_>,
    pool: &State<PgPool>,
    path: PathBuf
) -> Result<Template, Custom<String>> {
    if let Some(post) = PostFull::from_location(pool, &path.display().to_string()).await.map_err(to_internal_server_err)? {
        let context = PostPageContext {
            post_id: &Some(post.id),
            post_title: &post.title,
            post_content: &post.content,
            post_published: &post.published_at,
            post_author_id: post.author_id,
            post_author_name: &post.author_name,
            post_author_email: &post.author_email,
            post_created_at: &Some(post.created_at),
            post_last_editor_id: post.last_editor_id,
            post_last_editor_name: &post.last_editor_name,
            post_last_editor_email: &post.last_editor_email,
            post_last_edited_at: &Some(post.last_edited_at),
            common: common_context,
            page: PageContext {
                scripted: true,
                title: &post.title,
                template_name: POST_TEMPLATE
            }
        };
        Ok(Template::render(POST_TEMPLATE, context))
    } else {
        Err(Custom(Status::NotFound, "not found".to_string()))
    }
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
) -> Result<Created<Json<PostFull>>, Custom<String>> {
    if !login.is_editor() {
        return Err(Custom(Status::Forbidden, "admin or editor required".to_string()));
    }
    let update = update.0;
    let post = PostFull::create(pool, update.title, login.uid, login.name, login.email, update.published, update.content)
        .await
        .map_err(to_internal_server_err)?;

    Ok(Created::new(post.get_location()).body(Json(post)))
}
enum PutPostResponse {
    ChangedTitle(PostFull),
    UnchangedTitle
}
impl <'r> rocket::response::Responder<'r, 'static> for PutPostResponse {
    fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'static> {
        match self {
            Self::ChangedTitle(p) => {
                Created::new(p.get_location()).body(Json(p)).respond_to(req)
            },
            Self::UnchangedTitle => NoContent.respond_to(req)
        }        
    }
}

#[put("/posts/<path..>", data = "<update>")]
async fn put_post(
    pool: &State<PgPool>,
    login: LoginSession,
    path: PathBuf,
    update: Json<SavePost>
) -> Result<PutPostResponse, Custom<String>> {
    if !login.is_editor() {
        return Err(Custom(Status::Forbidden, "admin or editor required".to_string()));
    }

    let id = path.display().to_string()
        .parse::<i64>()
        .map_err(|e| Custom(Status::UnprocessableEntity, format!("Failed to parse input path {} to int: {e}", path.display())))?;
    if let Some(mut post) = PostFull::query_by_id(pool, id).await.map_err(to_internal_server_err)? {
        let updated_title = post.title != update.title;
        post.title = update.title.clone();
        post.content = update.content.clone();
        post.last_editor_id = login.uid;
        post.last_editor_name = login.name;
        post.last_editor_email = login.email;
        post.last_edited_at = Utc::now().fixed_offset();
        post.published_at = update.published.clone();
        
        post.save(pool).await
            .map_err(to_internal_server_err)?;
        
        if updated_title {
            Ok(PutPostResponse::ChangedTitle(post))
        } else {
            Ok(PutPostResponse::UnchangedTitle)
        }
    } else {
        Err(Custom(Status::NotFound, "not found".to_string()))
    }
}

#[delete("/posts/<path..>")]
async fn delete_post(
    pool: &State<PgPool>,
    login: LoginSession,
    path: PathBuf
) -> Result<NoContent, Custom<String>> {
    if !login.is_editor() {
        return Err(Custom(Status::Forbidden, "admin or editor required".to_string()));
    }
    let id = path.display().to_string()
        .parse::<i64>()
        .map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?;
    if let Some(post) = PostFull::query_by_id(pool, id).await.map_err(to_internal_server_err)? {
        post.delete(pool)
            .await
            .map_err(to_internal_server_err)
            .map(|_| NoContent)
    } else {
         Err(Custom(Status::NotFound, "not found".to_string()))
    }
}

#[derive(Debug, Serialize)]
struct 
PostPageContext<'r> {
    common: CommonPageContext<'r>,
    page: PageContext<'r>,
    post_id: &'r Option<i64>,
    post_title: &'r str,
    post_content: &'r str,
    post_author_id: i64,
    post_author_name: &'r str,
    post_author_email: &'r str,
    post_created_at: &'r Option<DateTime<FixedOffset>>,
    post_last_editor_id: i64,
    post_last_editor_name: &'r str,
    post_last_editor_email: &'r str,
    post_last_edited_at: &'r Option<DateTime<FixedOffset>>,
    post_published: &'r Option<DateTime<FixedOffset>>
}

#[get("/edit/<path..>")]
async fn get_post_editor(
    common_context: CommonPageContext<'_>,
    pool: &State<PgPool>,
    path: PathBuf,
    login: LoginSession
) -> Result<Template, Custom<String>> {
    if !login.is_editor() {
        return Err(Custom(Status::Forbidden, "admin or editor required".to_string()));
    }
    let path_str = path.display().to_string();
    if "new" == path_str {
        let context = PostPageContext {
            post_id: &None,
            post_title: "",
            post_content: "",
            post_author_id: login.uid,
            post_author_name: &login.name,
            post_author_email: &login.email,
            post_created_at: &None,
            post_last_editor_id: login.uid,
            post_last_editor_name: &login.name,
            post_last_editor_email: &login.email,
            post_last_edited_at: &None,
            post_published: &None,
            common: common_context,
            page: PageContext {
                scripted: true,
                title: "Create Post",
                template_name: EDIT_POST_TEMPLATE
            }
        };
        Ok(Template::render(EDIT_POST_TEMPLATE, context))
    } else if let Some(post) = PostFull::from_location(pool, &path_str).await.map_err(to_internal_server_err)? {
        let context = PostPageContext {
            post_id: &Some(post.id),
            post_title: &post.title,
            post_content: &post.content,
            post_author_id: post.author_id,
            post_author_name: &post.author_name,
            post_author_email: &post.author_email,
            post_created_at: &Some(post.created_at),
            post_last_editor_id: post.last_editor_id,
            post_last_editor_name: &post.last_editor_name,
            post_last_editor_email: &post.last_editor_email,
            post_last_edited_at: &Some(post.last_edited_at),
            post_published: &post.published_at,
            common: common_context,
            page: PageContext {
                scripted: true,
                title: &format!("Edit Post – {}", post.title),
                template_name: EDIT_POST_TEMPLATE
            }
        };
        info!("Loading template {} with context {context:?}", EDIT_POST_TEMPLATE);
        Ok(Template::render(EDIT_POST_TEMPLATE, context))
    } else {
        Err(Custom(Status::NotFound, "not found".to_string()))
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct PostBlobResult {
    location: String
}

#[post("/blobs", data = "<data>")]
async fn post_blob(
    pool: &State<PgPool>,
    content_type: &ContentType,
    data: Data<'_>
) -> Result<Created<Json<PostBlobResult>>, Custom<String>> {
    use tokio_util::io::ReaderStream;

    info!("Parsing POST wth content-type: {content_type:?}");
    let boundary = parse_boundary(&content_type.to_string())
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    info!("Multipart boundary is {boundary}");

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
        info!("Created Large Object with id {loid:?}");

        // Open the large object for writing
        let fd: i32 = sqlx::query_scalar("SELECT lo_open($1, 131072)") // 131072 = INV_WRITE
            .bind(loid)
            .fetch_one(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;
        // Start digest hasher
        let mut hasher = Sha256::new();

        // Stream the field’s contents
        let mut size: usize = 0;
        while let Some(chunk) = field.chunk().await
                .map_err(|e| Custom(Status::InternalServerError, e.to_string()))? {
            info!("Read chunk of {} bytes", chunk.len());
            size += chunk.len();

            // Write bytes to the Large Object
            let bytes = chunk.to_vec();
            hasher.update(&bytes);
            query("SELECT lowrite($1, $2)")
                .bind(fd)
                .bind(&bytes[..chunk.len()])
                .execute(&mut *tx)
                .await
                .map_err(to_internal_server_err)?;
        }
        // Close the large object
        sqlx::query("SELECT lo_close($1)")
            .bind(fd)
            .execute(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;

        let name = field.name().unwrap_or("unknown");
        let file_name = field.file_name().unwrap_or("unnamed");
        let mime_type = field.content_type()
            .map(|m| m.essence_str())
            .unwrap_or("application/octet-stream");
        println!("Processing field: {name} (filename: {file_name}, content-type: {mime_type})");

        let hash = hasher.finalize();
        let hash_hex = hex::encode(hash);
        info!("Generated hash {hash_hex}");

        // Insert metadata row
        let id: Option<String> = query_scalar("INSERT INTO blobs (id, mime_type, oid, size, created) VALUES ($1, $2, $3, $4, NOW()) ON CONFLICT DO NOTHING RETURNING id")
            .bind(&hash_hex)
            .bind(&mime_type)
            .bind(&loid)
            .bind(i64::try_from(size).map_err(to_internal_server_err)?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(to_internal_server_err)?;

        // If the insert did not return a row, the record for this hash already exists. We need to delete the duplicate Large Object.
        if id.is_none() {
            info!("Deleting duplicate large object id {loid:?}");
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
        let result = PostBlobResult {
            location: format!("/blog/blobs/{hash_hex}")
        };
        Ok(Created::new(location).body(Json(result)))
    } else {
        Err(Custom(Status::BadRequest, "missing multipart file".to_string()))
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
        Custom<String>
    > {
    let metadata: BlobMetadata = query_as("SELECT * FROM blobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| Custom(Status::NotFound, format!("blob with id {id} not found")))?;
    let media_type = MediaType::parse_flexible(&metadata.mime_type)
        .ok_or_else(|| Custom(Status::UnsupportedMediaType, metadata.mime_type))?;

    let mut tx = pool.begin()
        .await
        .map_err(to_internal_server_err)?;

    let lo_fd: i32 = sqlx::query_scalar("SELECT lo_open($1, 262144)") // 262144 = 0x40000 = INV_READ
        .bind(&metadata.oid)
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
                    if chunk.len() == 0 {
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