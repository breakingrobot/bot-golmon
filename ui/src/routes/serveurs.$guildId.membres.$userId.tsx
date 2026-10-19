import { createFileRoute } from '@tanstack/react-router'

import { ProfilePage } from '@/pages/members/ProfilePage'

export const Route = createFileRoute('/serveurs/$guildId/membres/$userId')({ component: ProfilePage })
