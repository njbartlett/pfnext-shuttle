// User account calls (backend src/users.rs and src/loginsession.rs).
import { apiRequest } from './api'
import type { LoggedInUser, NewUserRequest, UserPatch, UserRecord, UserUpdate } from './types'

export interface UserFilter {
  name?: string
  email?: string
  role?: string
}

export async function login(email: string, password: string, client?: string): Promise<LoggedInUser> {
  return apiRequest<LoggedInUser>('/login', {
    method: 'POST',
    body: { email, password, client },
    skipUnauthorizedHandler: true
  })
}

export async function logout(): Promise<void> {
  await apiRequest<void>('/logout', { method: 'POST' })
}

export async function getUser(userId: number): Promise<UserRecord> {
  return apiRequest<UserRecord>(`/users/${userId}`)
}

// Admin only
export async function listUsers(filter: UserFilter = {}): Promise<UserRecord[]> {
  return apiRequest<UserRecord[]>('/users/list', { query: { ...filter } })
}

export async function patchUser(userId: number, patch: UserPatch): Promise<void> {
  await apiRequest<unknown>(`/users/${userId}`, { method: 'PATCH', body: patch })
}

// Admin only
export async function updateUser(userId: number, update: UserUpdate): Promise<void> {
  await apiRequest<unknown>(`/users/${userId}`, { method: 'PUT', body: update })
}

export async function deleteUser(userId: number, password: string | null, websiteUrl: string): Promise<void> {
  await apiRequest<unknown>(`/users/${userId}`, {
    method: 'DELETE',
    body: { password, website_url: websiteUrl }
  })
}

export async function registerUser(request: NewUserRequest): Promise<void> {
  await apiRequest<unknown>('/register_user', { method: 'POST', body: request })
}

// Emails a temporary password; resetUrl is the page the email links to
export async function requestPasswordReset(email: string, websiteUrl: string, resetUrl: string): Promise<string> {
  const response = await apiRequest<unknown>('/request_pwd_reset', {
    method: 'POST',
    body: { email, website_url: websiteUrl, reset_url: resetUrl }
  })
  return typeof response === 'string' ? response : ''
}

export async function resetPassword(
  email: string,
  tempPassword: string,
  newPassword: string,
  websiteUrl: string
): Promise<void> {
  await apiRequest<unknown>('/reset_pwd', {
    method: 'POST',
    body: { email, temp_password: tempPassword, new_password: newPassword, website_url: websiteUrl }
  })
}
