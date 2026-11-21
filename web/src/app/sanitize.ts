// Rich text written by editors, trainers and admins (blog posts, session notes,
// challenge descriptions) is shown with v-html. The server stores it as sent, so
// it must be sanitised here: otherwise a script in it would run with the
// permissions of whoever views it, including admins.
import DOMPurify from 'dompurify'

export function sanitizeHtml(html: string | null | undefined): string {
  return html ? DOMPurify.sanitize(html) : ''
}
