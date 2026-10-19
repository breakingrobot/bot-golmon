import { type TransactionFilters } from '@/api/queries'

export type TransactionsSearch = TransactionFilters & { page?: number }
