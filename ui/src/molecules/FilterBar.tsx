import { type ReactNode } from 'react'

export function FilterBar({ children, onSubmit }: { children: ReactNode; onSubmit: () => void }) {
  return (
    <form
      className="grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-2 lg:grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit()
      }}
    >
      {children}
    </form>
  )
}
