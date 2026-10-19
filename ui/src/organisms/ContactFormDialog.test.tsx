import { screen, waitFor } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { Button } from '@/atoms/Button'
import { ContactFormDialog } from '@/organisms/ContactFormDialog'
import { jsonResponse, renderInGuild, TEST_GUILD } from '@/test/render'

const openDialog = async () => {
  const user = userEvent.setup()
  renderInGuild(<ContactFormDialog guildId={TEST_GUILD.id} trigger={<Button>Nouveau contact</Button>} />)
  await user.click(screen.getByRole('button', { name: 'Nouveau contact' }))
  return user
}

describe('ContactFormDialog', () => {
  it("refuse un nom vide sans appeler l'API", async () => {
    const fetchMock = vi.fn<typeof fetch>()
    vi.stubGlobal('fetch', fetchMock)
    const user = await openDialog()

    await user.type(screen.getByLabelText('Nom'), '   ')
    await user.click(screen.getByRole('button', { name: 'Ajouter' }))

    expect(await screen.findByText('Le nom est obligatoire.')).toBeVisible()
    expect(screen.getByLabelText('Nom')).toHaveAttribute('aria-invalid', 'true')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('limite la longueur du nom', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>())
    const user = await openDialog()

    await user.type(screen.getByLabelText('Nom'), 'x'.repeat(81))
    await user.click(screen.getByRole('button', { name: 'Ajouter' }))

    expect(await screen.findByText('80 caractères au plus.')).toBeVisible()
  })

  it('envoie les champs remplis, nettoyés, et ferme le dialogue', async () => {
    const fetchMock = vi.fn<typeof fetch>(() =>
      jsonResponse({ id: '9', name: 'Ballas', kind: 'GROUPE', relation: 'HOSTILE', phone: '555-0101' }, 201),
    )
    vi.stubGlobal('fetch', fetchMock)
    const user = await openDialog()

    await user.type(screen.getByLabelText('Nom'), '  Ballas ')
    await user.click(screen.getByRole('combobox', { name: 'Relation' }))
    await user.click(await screen.findByRole('option', { name: 'Hostile' }))
    await user.type(screen.getByLabelText('Téléphone'), '555-0101')
    await user.click(screen.getByRole('button', { name: 'Ajouter' }))

    await waitFor(() => expect(fetchMock).toHaveBeenCalledOnce())
    const [url, init] = fetchMock.mock.calls[0] ?? []
    expect(url).toBe(`/api/guilds/${TEST_GUILD.id}/contacts`)
    expect(JSON.parse(String(init?.body))).toStrictEqual({
      name: 'Ballas',
      kind: 'GROUPE',
      relation: 'HOSTILE',
      phone: '555-0101',
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })
})
