import { createFileRoute } from '@tanstack/react-router'

import { ConfigurationPage } from '@/pages/configuration/ConfigurationPage'

export const Route = createFileRoute('/serveurs/$guildId/configuration')({ component: ConfigurationPage })
