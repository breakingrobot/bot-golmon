import { createFileRoute } from '@tanstack/react-router'

import { StaffPage } from '@/pages/tickets/StaffPage'

export const Route = createFileRoute('/tickets/staff')({ component: StaffPage })
