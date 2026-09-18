// The members an "act on behalf of" picker offers: every member for admins
// and trainers (who the backend lets list users), only the logged-in user
// otherwise. Replaces loadAllUsers() in library.js.
import { listUsers, type UserSummary } from '@pfnext/shared'
import { isAdmin, isTrainer, user } from '@/stores/auth'

export async function loadSelectableMembers(): Promise<UserSummary[]> {
  if (isAdmin.value || isTrainer.value) {
    return listUsers()
  }
  return user.value ? [user.value] : []
}
