// Ancienne adresse : /logs/staff → /tickets/staff.

import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/logs/staff')({
  beforeLoad: () => {
    throw redirect({ to: '/tickets/staff', replace: true })
  },
})
