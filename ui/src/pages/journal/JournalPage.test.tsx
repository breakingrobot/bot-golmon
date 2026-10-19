// Page complète (routeur, requêtes, mise en page du serveur) servie par la maquette de l'API.

import { QueryClientProvider } from '@tanstack/react-query'
import { createMemoryHistory, RouterProvider } from '@tanstack/react-router'
import { render, screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createAppRouter } from '@/router'
import { createTestQueryClient } from '@/test/render'

const GUILD = '1187000000000000042'

/** Entrées du journal : la liste ordonnée de la page (une action par élément). */
const entries = () => {
  const list = screen.getByRole('main').querySelector('ol')
  if (!list) throw new Error('liste du journal absente')
  return within(list).getAllByRole('listitem')
}

const renderApp = (path: string) => {
  const router = createAppRouter(createTestQueryClient(), createMemoryHistory({ initialEntries: [path] }))
  render(
    <QueryClientProvider client={router.options.context.queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )
  return router
}

describe('page du journal (maquette)', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('liste les actions du staff dans la mise en page du serveur', async () => {
    renderApp(`/serveurs/${GUILD}/journal`)

    expect(await screen.findByRole('heading', { level: 1, name: 'Journal du staff' }, { timeout: 5000 })).toBeVisible()
    expect(screen.getByRole('navigation', { name: 'Pages du serveur' })).toBeInTheDocument()
    expect(await screen.findByText(/^Page 1 sur \d+$/u, {}, { timeout: 5000 })).toBeVisible()
    expect(entries().length).toBeGreaterThan(5)
  })

  it("filtre par action en écrivant le filtre dans l'adresse", async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${GUILD}/journal`)
    await screen.findByText(/^Page 1 sur \d+$/u, {}, { timeout: 5000 })

    await user.click(screen.getByRole('combobox', { name: 'Action' }))
    await user.click(await screen.findByRole('option', { name: 'Véto posé' }))

    await waitFor(() => expect(router.state.location.search).toStrictEqual({ action: 'recrue.veto' }))
    await waitFor(
      () => {
        const items = entries()
        expect(items.length).toBeGreaterThan(0)
        for (const item of items) expect(item).toHaveTextContent('Véto posé')
      },
      { timeout: 5000 },
    )
  })
})
