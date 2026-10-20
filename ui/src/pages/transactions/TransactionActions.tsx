// Actions sur une transaction non clôturée (`transaction.utiliser`) : statut, objet, annulation.

import { Ban, Plus } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { useAddTransactionItem, useCancelTransaction, useSetTransactionStatus } from '@/api/queries-groupe'
import type { Transaction, TransactionStatus } from '@/api/types'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/atoms/AlertDialog'
import { Button } from '@/atoms/Button'
import { Input } from '@/atoms/Input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { itemsOf } from '@/pages/transactions/NewTransactionDialog'

export type TransactionActionsProps = {
  guildId: string
  transaction: Transaction
  /** Statuts proposés (sans « Annulée », qui a son bouton). */
  statuses: { value: TransactionStatus; label: string }[]
}

export const TransactionActions = ({ guildId, transaction, statuses }: TransactionActionsProps) => {
  const setStatus = useSetTransactionStatus(guildId)
  const addItem = useAddTransactionItem(guildId)
  const cancel = useCancelTransaction(guildId)
  const [nom, setNom] = useState('')
  const [quantite, setQuantite] = useState('1')
  const [prix, setPrix] = useState('')
  const [error, setError] = useState<string | null>(null)
  const { id } = transaction

  const submitItem = (event: FormEvent) => {
    event.preventDefault()
    const items = itemsOf([{ key: 0, nom, quantite, prix }])
    const item = items?.[0]
    if (!item) {
      setError(
        items === null ? 'Quantité entière (1 au moins) et prix unitaire valide.' : "Indiquez le nom de l'objet.",
      )
      return
    }
    setError(null)
    addItem.mutate(
      { id, item },
      {
        onSuccess: () => {
          setNom('')
          setQuantite('1')
          setPrix('')
        },
      },
    )
  }

  return (
    <div className="mt-3 grid gap-3 border-t pt-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={transaction.status === 'ANNULEE' ? '' : transaction.status}
          onValueChange={(status) => setStatus.mutate({ id, status: status as TransactionStatus })}
          disabled={setStatus.isPending}
        >
          <SelectTrigger className="w-full sm:w-64" aria-label="Changer le statut">
            <SelectValue placeholder="Changer le statut" />
          </SelectTrigger>
          <SelectContent>
            {statuses.map((status) => (
              <SelectItem key={status.value} value={status.value}>
                {status.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {transaction.status !== 'ANNULEE' && (
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" size="sm" className="text-danger hover:text-danger">
                <Ban aria-hidden /> Annuler la transaction
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Annuler la transaction avec {transaction.groupe} ?</AlertDialogTitle>
                <AlertDialogDescription>
                  Elle passe en « Annulée » : elle reste dans l'historique, hors des montants. La fiche Discord et le
                  tableau de bord sont mis à jour.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Retour</AlertDialogCancel>
                <AlertDialogAction disabled={cancel.isPending} onClick={() => cancel.mutate(id)}>
                  Annuler la transaction
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        )}
      </div>
      <form className="grid grid-cols-[1fr_4.5rem] gap-2 sm:grid-cols-[1fr_4.5rem_7rem_auto]" onSubmit={submitItem}>
        <Input value={nom} onChange={(event) => setNom(event.target.value)} placeholder="Objet" aria-label="Objet" />
        <Input
          value={quantite}
          inputMode="numeric"
          onChange={(event) => setQuantite(event.target.value)}
          aria-label="Quantité"
        />
        <Input
          value={prix}
          inputMode="decimal"
          onChange={(event) => setPrix(event.target.value)}
          placeholder="Prix / u"
          aria-label="Prix unitaire"
        />
        <Button type="submit" variant="outline" disabled={addItem.isPending}>
          <Plus aria-hidden /> Ajouter
        </Button>
        {error && (
          <p className="col-span-full text-xs text-danger" role="alert">
            {error}
          </p>
        )}
      </form>
    </div>
  )
}
