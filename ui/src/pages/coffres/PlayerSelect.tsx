import { useQuery } from '@tanstack/react-query'

import { coffrePlayersQuery } from '@/api/queries'
import type { CoffreMeta } from '@/api/types'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { ALL, fromSelect } from '@/lib/search'
import { Field } from '@/molecules/Field'

export function PlayerSelect({
  guildId,
  meta,
  value,
  jours,
  onChange,
}: {
  guildId: string
  meta: CoffreMeta | undefined
  value: string | undefined
  jours: string
  onChange: (joueur: string | undefined) => void
}) {
  // Personnages vus sur la période, plus les personnages reliés à un membre.
  const players = useQuery(coffrePlayersQuery(guildId, { jours }))
  const options = new Map<string, string>()
  for (const link of meta?.links ?? []) options.set(link.gameId, `${link.name} (${link.member.name})`)
  for (const player of players.data ?? []) {
    if (player.gameId && !options.has(player.gameId)) options.set(player.gameId, player.name)
  }
  if (value && !options.has(value)) options.set(value, `Personnage ${value}`)
  return (
    <Field label="Joueur">
      {(props) => (
        <Select value={value ?? ALL} onValueChange={(next) => onChange(fromSelect(next))}>
          <SelectTrigger {...props}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Tous les joueurs</SelectItem>
            {[...options.entries()]
              .sort((a, b) => a[1].localeCompare(b[1], 'fr'))
              .map(([gameId, label]) => (
                <SelectItem key={gameId} value={gameId}>
                  {label}
                </SelectItem>
              ))}
          </SelectContent>
        </Select>
      )}
    </Field>
  )
}
