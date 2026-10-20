// Provisoire : remplacée par la page des absences à l'intégration.

import { createFileRoute } from '@tanstack/react-router'

import { SoonPage } from '@/pages/soon/SoonPage'

export const Route = createFileRoute('/serveurs/$guildId/absences')({ component: SoonPage })
