import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import { useEffect, useState } from 'react'

import { Input } from '@/atoms/Input'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { useDebounced } from '@/hooks/use-debounced'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { PageHeader } from '@/molecules/PageHeader'
import { TABS } from '@/pages/annuaire/annuaire-search'
import type { Tab } from '@/pages/annuaire/annuaire-search'
import { PhonesSection } from '@/pages/annuaire/PhonesSection'
import { VehiclesSection } from '@/pages/annuaire/VehiclesSection'

const route = getRouteApi('/serveurs/$guildId/annuaire')

export const AnnuaireView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/annuaire' })
  const [text, setText] = useState(search.q ?? '')
  const q = useDebounced(text.trim(), 300)
  const canVehicles = hasPerm(guild, 'vehicule.utiliser')
  const canPhones = guild.inGroup || guild.isAdmin
  const tabs = TABS.filter((tab) => (tab.value === 'vehicules' ? canVehicles : canPhones))
  const fallback: Tab = canVehicles ? 'vehicules' : 'numeros'
  const tab: Tab = tabs.some((item) => item.value === search.onglet) ? (search.onglet ?? fallback) : fallback

  useEffect(() => {
    if ((search.q ?? '') !== q) {
      void navigate({ search: (prev) => compact({ ...prev, q: q || undefined }), replace: true })
    }
  }, [q, search.q, navigate])

  return (
    <>
      <PageHeader title="Annuaire" description={`${guild.name} · véhicules du groupe et numéros des membres`} />
      <div className="mb-4 flex flex-wrap items-center gap-3">
        {tabs.length > 1 && (
          <ToggleGroup
            type="single"
            value={tab}
            onValueChange={(value) => {
              if (value) void navigate({ search: (prev) => compact({ ...prev, onglet: value as Tab }), replace: true })
            }}
            aria-label="Vue"
          >
            {tabs.map((item) => (
              <ToggleGroupItem key={item.value} value={item.value}>
                {item.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        )}
        <div className="relative w-full sm:w-72">
          <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-ink-subtle" aria-hidden />
          <Input
            type="search"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={tab === 'vehicules' ? 'Plaque, modèle, propriétaire' : 'Nom ou numéro'}
            aria-label="Recherche"
            className="pl-9"
          />
        </div>
      </div>
      {tab === 'vehicules' ? <VehiclesSection guildId={guild.id} q={q} /> : <PhonesSection guildId={guild.id} q={q} />}
    </>
  )
}
