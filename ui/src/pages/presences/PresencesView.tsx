import { getRouteApi, useNavigate } from '@tanstack/react-router'

import { csvUrl } from '@/api/client'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import type { SearchPatch } from '@/lib/search'
import { CsvButton } from '@/molecules/CsvButton'
import { PageHeader } from '@/molecules/PageHeader'
import { AttendanceSection } from '@/pages/presences/AttendanceSection'
import { NewPresenceDialog } from '@/pages/presences/NewPresenceDialog'
import { PresenceList } from '@/pages/presences/PresenceList'
import { TABS } from '@/pages/presences/presences-search'
import type { PresencesSearch, Tab } from '@/pages/presences/presences-search'
import { PresenceSheet } from '@/pages/presences/PresenceSheet'
import { SchedulesSection } from '@/pages/presences/SchedulesSection'

const route = getRouteApi('/serveurs/$guildId/presences')

export const PresencesView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/presences' })
  const canCreate = hasPerm(guild, 'presence.creer')
  const canManage = hasPerm(guild, 'presence.gerer')
  const tabs = TABS.filter((tab) => tab.value !== 'plannings' || canManage)
  const tab: Tab = search.onglet === 'plannings' && !canManage ? 'liste' : (search.onglet ?? 'liste')
  const jours = search.jours ?? '30'
  const setSearch = (patch: SearchPatch<PresencesSearch>) =>
    void navigate({ search: (prev) => compact({ ...prev, ...patch }), replace: true })

  return (
    <>
      <PageHeader
        title="Présences"
        description={`${guild.name} · réponses, assiduité et présences récurrentes`}
        actions={
          <>
            {tab === 'assiduite' && <CsvButton href={csvUrl(`/guilds/${guild.id}/presences/export.csv`, { jours })} />}
            {canCreate && (
              <NewPresenceDialog guildId={guild.id} onCreated={(id) => setSearch({ onglet: 'liste', presence: id })} />
            )}
          </>
        }
      />

      <ToggleGroup
        type="single"
        value={tab}
        onValueChange={(value) => {
          if (value) setSearch({ onglet: value === 'liste' ? undefined : (value as Tab), page: undefined })
        }}
        aria-label="Vue"
        className="mb-4"
      >
        {tabs.map((item) => (
          <ToggleGroupItem key={item.value} value={item.value}>
            {item.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>

      {tab === 'liste' && (
        <PresenceList
          guildId={guild.id}
          search={search}
          onSearch={setSearch}
          onOpen={(presence) => setSearch({ presence })}
        />
      )}
      {tab === 'assiduite' && (
        <AttendanceSection guildId={guild.id} jours={jours} onPeriod={(value) => setSearch({ jours: value })} />
      )}
      {tab === 'plannings' && <SchedulesSection guildId={guild.id} />}

      <PresenceSheet
        guildId={guild.id}
        presenceId={search.presence}
        onClose={() => setSearch({ presence: undefined })}
      />
    </>
  )
}
