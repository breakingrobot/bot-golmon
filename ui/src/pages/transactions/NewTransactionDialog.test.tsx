import { screen, waitFor } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { itemsOf, NewTransactionDialog, parsePrice } from '@/pages/transactions/NewTransactionDialog'
import { jsonResponse, renderInGuild, TEST_GUILD } from '@/test/render'

describe('saisie d’une transaction', () => {
  it('lit les prix à la française', () => {
    expect(parsePrice('1 500')).toBe(1500)
    expect(parsePrice('12,5 $')).toBe(12.5)
    expect(parsePrice('-3')).toBeNull()
    expect(parsePrice('1,234')).toBeNull()
  })

  it('ignore les objets sans nom et refuse une quantité invalide', () => {
    expect(itemsOf([{ key: 1, nom: ' Kit ', quantite: '', prix: '' }])).toStrictEqual([{ nom: 'Kit', quantite: 1 }])
    expect(itemsOf([{ key: 1, nom: '', quantite: 'x', prix: '' }])).toStrictEqual([])
    expect(itemsOf([{ key: 1, nom: 'Kit', quantite: '0', prix: '' }])).toBeNull()
  })

  it('envoie la transaction et ses objets', async () => {
    const fetchMock = vi.fn<typeof fetch>(() =>
      jsonResponse({ id: '1', groupe: 'Ballas', type: 'ACHAT', prix: 1500, statusLabel: 'Préparation' }, 201),
    )
    vi.stubGlobal('fetch', fetchMock)
    const user = userEvent.setup()
    renderInGuild(<NewTransactionDialog guildId={TEST_GUILD.id} />)

    await user.click(screen.getByRole('button', { name: 'Nouvelle transaction' }))
    await user.type(screen.getByLabelText('Groupe'), 'Ballas')
    await user.click(screen.getByRole('radio', { name: 'Achat' }))
    await user.type(screen.getByLabelText('Prix total ($)'), '1 500')
    await user.click(screen.getByRole('button', { name: 'Ajouter un objet' }))
    await user.type(screen.getByLabelText("Nom de l'objet 1"), 'Kit de réparation')
    await user.click(screen.getByRole('button', { name: 'Créer' }))

    await waitFor(() => expect(fetchMock).toHaveBeenCalledOnce())
    const [url, init] = fetchMock.mock.calls[0] ?? []
    expect(url).toBe(`/api/guilds/${TEST_GUILD.id}/transactions`)
    expect(JSON.parse(String(init?.body))).toStrictEqual({
      groupe: 'Ballas',
      type: 'ACHAT',
      prix: 1500,
      items: [{ nom: 'Kit de réparation', quantite: 1 }],
    })
  })
})
