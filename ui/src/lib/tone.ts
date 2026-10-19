export type Tone = 'default' | 'user' | 'staff' | 'internal' | 'danger' | 'brand' | 'muted'

export const toneClass: Record<Tone, string> = {
  default: 'text-ink',
  user: 'text-user',
  staff: 'text-staff',
  internal: 'text-internal',
  danger: 'text-danger',
  brand: 'text-brand',
  muted: 'text-ink-muted',
}

export function attendanceTone(percent: number): 'staff' | 'internal' | 'danger' {
  if (percent >= 75) return 'staff'
  if (percent >= 50) return 'internal'
  return 'danger'
}
