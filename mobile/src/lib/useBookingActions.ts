// Book / cancel / waitlist actions with the user-facing dialogs, shared by
// the sessions list and the session detail page.
import { alertController, toastController } from '@ionic/vue'
import * as service from './bookingService'
import type { Session } from '@pfnext/shared'

export function useBookingActions(refresh: () => Promise<void>) {
  async function toast(message: string, color: 'success' | 'danger' = 'success') {
    const t = await toastController.create({ message, duration: 2500, color, position: 'bottom' })
    await t.present()
  }

  async function reportError(e: unknown) {
    await toast(e instanceof Error ? e.message : 'Something went wrong', 'danger')
  }

  async function book(session: Session, creditsUsed = 0) {
    try {
      const result = await service.bookSession(session.id, creditsUsed, session.cost)
      if (result.outcome === 'credits_required') {
        await confirmCredits(session)
        return
      }
      await toast('Booked!')
      await refresh()
    } catch (e) {
      await reportError(e)
    }
  }

  // The 402 credits opt-in handshake: confirm, then re-book spending credits
  async function confirmCredits(session: Session) {
    const alert = await alertController.create({
      header: 'Use credits?',
      message: `This booking costs ${session.cost} credit${session.cost === 1 ? '' : 's'} from your Pay As You Go balance.`,
      buttons: [
        { text: 'Cancel', role: 'cancel' },
        {
          text: `Spend ${session.cost}`,
          role: 'confirm',
          handler: () => {
            void book(session, session.cost)
          }
        }
      ]
    })
    await alert.present()
  }

  async function cancel(session: Session) {
    const alert = await alertController.create({
      header: 'Cancel booking?',
      message: 'Any credits spent on this booking will be refunded.',
      buttons: [
        { text: 'Keep booking', role: 'cancel' },
        {
          text: 'Cancel booking',
          role: 'destructive',
          handler: () => {
            void (async () => {
              try {
                await service.cancelBooking(session.id)
                await toast('Booking cancelled')
                await refresh()
              } catch (e) {
                await reportError(e)
              }
            })()
          }
        }
      ]
    })
    await alert.present()
  }

  async function joinWaitlist(session: Session) {
    try {
      await service.joinWaitlist(session.id)
      await toast('Added to the waitlist — we’ll email you if a space opens up')
      await refresh()
    } catch (e) {
      await reportError(e)
    }
  }

  async function leaveWaitlist(session: Session) {
    try {
      await service.leaveWaitlist(session.id)
      await toast('Removed from the waitlist')
      await refresh()
    } catch (e) {
      await reportError(e)
    }
  }

  return { book, cancel, joinWaitlist, leaveWaitlist }
}
