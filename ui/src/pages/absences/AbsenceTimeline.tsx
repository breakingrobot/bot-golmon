// Frise des absences : une ligne par membre, une colonne par jour (~5 semaines), aujourd'hui marqué.
// Une seule barre est atteignable par Tab ; les flèches, Début et Fin passent d'une barre à l'autre.

import { useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'

import type { AbsenceEntry } from '@/api/types'
import { moveFocus, parisDayOf, timelineRows } from '@/lib/absence-timeline'
import type { TimelineFocus } from '@/lib/absence-timeline'
import { formatDay, formatShortDay } from '@/lib/format'
import { cn } from '@/lib/utils'
import { MemberAvatar } from '@/molecules/MemberAvatar'

const BAR_TONES = {
  current: 'bg-internal text-white',
  upcoming: 'bg-brand text-white',
  past: 'bg-muted text-ink-muted',
} as const

const WEEKDAY_INITIALS = ['L', 'M', 'M', 'J', 'V', 'S', 'D']

export type AbsenceTimelineProps = {
  absences: AbsenceEntry[]
  days: string[]
  today: string
  now: string
  selectedId: string | null
  onSelect: (absence: AbsenceEntry) => void
}

export const AbsenceTimeline = ({ absences, days, today, now, selectedId, onSelect }: AbsenceTimelineProps) => {
  const rows = timelineRows(absences, days, now)
  const [focus, setFocus] = useState<TimelineFocus>({ row: 0, bar: 0 })
  const bars = useRef(new Map<string, HTMLButtonElement>())
  const columns = { gridTemplateColumns: `repeat(${days.length}, minmax(0, 1fr))` }
  const todayIndex = days.indexOf(today)

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, current: TimelineFocus) => {
    const next = moveFocus(rows, current, event.key)
    if (!next) return
    event.preventDefault()
    setFocus(next)
    bars.current.get(`${next.row}-${next.bar}`)?.focus()
  }

  if (rows.length === 0) {
    return <p className="px-4 py-6 text-center text-sm text-ink-muted">Aucune absence sur ces cinq semaines.</p>
  }

  return (
    <div role="group" aria-label={`Absences du ${formatDay(days[0] ?? today)} au ${formatDay(days.at(-1) ?? today)}`}>
      <div className="grid grid-cols-[10rem_1fr] border-b text-xs text-ink-subtle">
        <span className="px-3 py-2">Membre</span>
        <div className="grid" style={columns} aria-hidden>
          {days.map((day) => {
            const weekday = (new Date(`${day}T12:00:00Z`).getUTCDay() + 6) % 7
            const monday = weekday === 0
            return (
              <span
                key={day}
                className={cn(
                  'py-2 text-center tabular-nums',
                  monday && 'border-l',
                  day === today && 'rounded-t-md bg-brand-soft font-semibold text-brand',
                )}
                title={formatDay(day)}
              >
                {monday ? formatShortDay(day) : (WEEKDAY_INITIALS[weekday] ?? '')}
              </span>
            )
          })}
        </div>
      </div>
      <ul>
        {rows.map((row, rowIndex) => (
          <li key={row.member.id} className="grid grid-cols-[10rem_1fr] items-center border-b last:border-b-0">
            <span className="flex min-w-0 items-center gap-2 px-3 py-2 text-sm font-medium text-ink">
              <MemberAvatar member={row.member} className="size-5" />
              <span className="truncate">{row.member.name}</span>
            </span>
            <div className="relative grid h-10 items-center" style={columns}>
              {todayIndex !== -1 && (
                <span
                  aria-hidden
                  className="pointer-events-none absolute inset-y-0 w-px bg-brand"
                  style={{ left: `${((todayIndex + 0.5) / days.length) * 100}%` }}
                />
              )}
              {row.bars.map((bar, barIndex) => {
                const { absence } = bar
                const tabbable =
                  (focus.row === rowIndex && focus.bar === barIndex) ||
                  (!rows[focus.row]?.bars[focus.bar] && rowIndex === 0 && barIndex === 0)
                const label = `${absence.member.name}, du ${formatDay(parisDayOf(absence.from))} au ${formatDay(
                  parisDayOf(absence.to),
                )} : ${absence.motif}`
                return (
                  <button
                    key={absence.id}
                    ref={(element) => {
                      if (element) bars.current.set(`${rowIndex}-${barIndex}`, element)
                      else bars.current.delete(`${rowIndex}-${barIndex}`)
                    }}
                    type="button"
                    tabIndex={tabbable ? 0 : -1}
                    aria-label={label}
                    aria-pressed={selectedId === absence.id}
                    title={label}
                    onFocus={() => setFocus({ row: rowIndex, bar: barIndex })}
                    onKeyDown={(event) => onKeyDown(event, { row: rowIndex, bar: barIndex })}
                    onClick={() => onSelect(absence)}
                    className={cn(
                      'z-10 mx-px h-6 truncate px-1.5 text-left text-xs font-medium focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
                      BAR_TONES[bar.state],
                      bar.clippedStart ? 'rounded-l-none' : 'rounded-l-md',
                      bar.clippedEnd ? 'rounded-r-none' : 'rounded-r-md',
                      selectedId === absence.id && 'ring-2 ring-ink',
                    )}
                    style={{ gridColumn: `${bar.start + 1} / span ${bar.span}` }}
                  >
                    {absence.motif}
                  </button>
                )
              })}
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}
