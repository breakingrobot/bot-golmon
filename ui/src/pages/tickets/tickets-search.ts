export const STATUSES = ['open', 'closed', 'suspended'] as const

export type TicketsSearch = { q?: string; status?: (typeof STATUSES)[number]; page?: number }
