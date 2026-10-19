import { useNavigate } from '@tanstack/react-router'

import type { GuildAccess, Me } from '@/api/types'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { findGuild, visiblePages } from '@/lib/guild'
import { GuildIcon } from '@/molecules/GuildIcon'

export function GuildSwitcher({ me, guild }: { me: Me; guild: GuildAccess }) {
  const navigate = useNavigate()
  if (me.guilds.length < 2) {
    return (
      <div className="flex items-center gap-2.5 px-2">
        <GuildIcon guild={guild} size="sm" />
        <span className="truncate text-sm font-semibold text-ink">{guild.name}</span>
      </div>
    )
  }
  return (
    <Select
      value={guild.id}
      onValueChange={(guildId) => {
        const target = findGuild(me, guildId)
        const first = target ? visiblePages(target)[0] : undefined
        if (first?.to) void navigate({ to: first.to, params: { guildId } })
        else void navigate({ to: '/serveurs/$guildId/membres/$userId', params: { guildId, userId: me.user?.id ?? '' } })
      }}
    >
      <SelectTrigger aria-label="Changer de serveur" className="h-auto py-2">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {me.guilds.map((item) => (
          <SelectItem key={item.id} value={item.id}>
            <GuildIcon guild={item} size="sm" />
            <span className="truncate font-medium">{item.name}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
