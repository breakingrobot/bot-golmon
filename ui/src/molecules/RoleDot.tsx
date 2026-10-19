import { roleColor } from '@/lib/utils'

export function RoleDot({ color }: { color: number | null }) {
  const css = roleColor(color)
  return (
    <span
      className="inline-block size-2.5 shrink-0 rounded-full bg-ink-subtle"
      style={css ? { background: css } : undefined}
      aria-hidden
    />
  )
}
