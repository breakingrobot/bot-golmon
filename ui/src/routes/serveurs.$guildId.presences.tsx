// Provisoire : remplacée par la page des présences à l'intégration.

import { createFileRoute } from '@tanstack/react-router'

import { SoonPage } from '@/pages/soon/SoonPage'

export const Route = createFileRoute('/serveurs/$guildId/presences')({ component: SoonPage })
