// Website configuration of the shared API client: same-origin, unversioned
// mount, session cookie. Imported once by mountPage() so it runs before any
// component can issue a request.
import { configureApi } from '@pfnext/shared'

configureApi({ baseUrl: '/api', credentials: 'include' })
