// Blog posts (backend src/blog.rs). Posts are addressed by title: a post's
// page is /blog/posts/<title>.html, which is also what the legacy links in
// emails and social posts look like.
import { apiRequest } from './api'
import type { OrphanImage, Post, PostSummary, SavePost } from './types'

// Published posts, newest first; `all` (editors only) includes drafts and
// posts scheduled for the future
export async function listPosts(all = false): Promise<PostSummary[]> {
  return apiRequest<PostSummary[]>('/posts', { query: { all: all ? true : undefined } })
}

export async function getPost(postId: number): Promise<Post> {
  return apiRequest<Post>(`/posts/${postId}`)
}

export async function getPostByTitle(title: string): Promise<Post> {
  return apiRequest<Post>(`/posts/by-title/${encodeURIComponent(title)}`)
}

// Editors only. Resolves to the post as stored.
export async function createPost(post: SavePost): Promise<Post> {
  return apiRequest<Post>('/posts', { method: 'POST', body: post })
}

export async function updatePost(postId: number, post: SavePost): Promise<Post> {
  return apiRequest<Post>(`/posts/${postId}`, { method: 'PUT', body: post })
}

// Editors only: publishes the post at `published` (an ISO date-time), or
// unpublishes it when null, leaving its content alone
export async function setPostPublication(postId: number, published: string | null): Promise<Post> {
  return apiRequest<Post>(`/posts/${postId}/publication`, { method: 'PUT', body: { published } })
}

export async function deletePost(postId: number): Promise<void> {
  await apiRequest<void>(`/posts/${postId}`, { method: 'DELETE' })
}

// Editors only: uploaded images that no post embeds, oldest first. These
// include images added in the editor to a post that has not been saved yet.
export async function listOrphanImages(): Promise<OrphanImage[]> {
  return apiRequest<OrphanImage[]>('/images/orphans')
}

// Editors only; refused with 409 if a post has since embedded the image
export async function deleteOrphanImage(name: string): Promise<void> {
  await apiRequest<void>(`/images/orphans/${encodeURIComponent(name)}`, { method: 'DELETE' })
}

// Editors only: deletes every image that no post embeds at the time
export async function deleteAllOrphanImages(): Promise<void> {
  await apiRequest<void>('/images/orphans', { method: 'DELETE' })
}

// Logged-in users only: records that the user has read the post. Fetching a
// post does not, as the editor fetches it too. Repeats keep the first read.
export async function markPostRead(postId: number): Promise<void> {
  await apiRequest<void>(`/posts/${postId}/read`, { method: 'PUT' })
}

// The last path segment of a post's page, in decoded form: the router
// encodes it when building the URL and decodes it when reading the params
export function postLocation(title: string): string {
  return `${title}.html`
}

// The title a page location names, or null if it is not a post location
export function postTitle(location: string): string | null {
  return location.endsWith('.html') ? location.slice(0, -'.html'.length) : null
}
