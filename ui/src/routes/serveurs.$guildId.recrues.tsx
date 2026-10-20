// Provisoire : remplacée par la page des recrues à l'intégration.

import { createFileRoute } from '@tanstack/react-router'

import { SoonPage } from '@/pages/soon/SoonPage'

export const Route = createFileRoute('/serveurs/$guildId/recrues')({ component: SoonPage })
