import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { PageHeader } from '@/molecules/PageHeader'
import { AlertsSection } from '@/pages/configuration/AlertsSection'
import { ModulesStatus } from '@/pages/configuration/ModulesStatus'
import { ObjectiveSection } from '@/pages/configuration/ObjectiveSection'

export const ConfigurationPage = () => {
  const { guild } = useGuild()
  if (!guild.isAdmin) return <Forbidden>Réservé aux personnes qui peuvent « Gérer le serveur ».</Forbidden>
  return (
    <>
      <PageHeader title="Configuration" description={`${guild.name} · état des modules et réglages du site`} />
      <ModulesStatus />
      <ObjectiveSection />
      {hasPerm(guild, 'coffre.voir') && <AlertsSection />}
    </>
  )
}
