import { createFileRoute } from '@tanstack/react-router'

import { PermissionsPage } from '@/pages/permissions/PermissionsPage'

export const Route = createFileRoute('/serveurs/$guildId/permissions')({ component: PermissionsPage })
