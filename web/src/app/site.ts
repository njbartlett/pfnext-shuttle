// Site-wide constants. The Rust side has the same branding string in
// Config.toml for the Tera-rendered blog pages and emails; keep them in step.
export const SITE_NAME = 'Another Level'

// The blog is still server-rendered by src/blog.rs, so links to it are plain
// anchors (full page loads) rather than router links
export const BLOG_URL = '/blog/index.html'
export const NEW_BLOG_POST_URL = '/blog/edit/new'
