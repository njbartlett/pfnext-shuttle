// Client-side form validation matching what the backend accepts.

const EMAIL_PATTERN =
  /^(([^<>()[\]\\.,;:\s@"]+(\.[^<>()[\]\\.,;:\s@"]+)*)|.(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/

// Digits only, optional leading "+"; no spaces or punctuation
const PHONE_PATTERN = /^\+?\d+$/

export function isValidEmail(email: string | null | undefined): boolean {
  return EMAIL_PATTERN.test(String(email ?? '').toLowerCase())
}

export function isValidPhone(phone: string | null | undefined): boolean {
  return PHONE_PATTERN.test(String(phone ?? ''))
}

export const PHONE_FORMAT_MESSAGE =
  'Enter a valid phone number without space or parentheses (country code optional)'
export const EMAIL_FORMAT_MESSAGE = 'Enter a valid email address'
