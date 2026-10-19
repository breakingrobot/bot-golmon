import { Save } from 'lucide-react'
import { type FormEvent, useState } from 'react'

import { useSaveObjective } from '@/api/queries'
import type { Objective } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Card, CardContent, CardDescription, CardHeader } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { Label } from '@/atoms/Label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { Switch } from '@/atoms/Switch'
import { formatMoney } from '@/lib/format'
import { useGuild } from '@/lib/guild-context'
import { COUNTS_LABEL } from '@/lib/objective'
import { Field } from '@/molecules/Field'

export function ObjectiveForm({ initial }: { initial: Objective | null }) {
  const { guild } = useGuild()
  const mutation = useSaveObjective(guild.id)
  const [amount, setAmount] = useState(initial ? String(initial.amount) : '')
  const [counts, setCounts] = useState<Objective['counts']>(initial?.counts ?? 'total')
  const [reminder, setReminder] = useState(initial?.reminder ?? true)
  const [error, setError] = useState<string | null>(null)

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const value = Number(amount.replace(/[\s $]/g, ''))
    if (!Number.isInteger(value) || value <= 0) {
      setError('Montant entier et positif, en dollars.')
      return
    }
    setError(null)
    mutation.mutate({ amount: value, counts, reminder })
  }

  return (
    <Card>
      <CardHeader>
        <CardDescription>
          {initial
            ? `Actuellement : ${formatMoney(initial.amount)} par semaine (${COUNTS_LABEL[initial.counts]}).`
            : 'Aucun objectif configuré : pas de suivi ni de rappel.'}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form className="grid gap-4 md:grid-cols-[1fr_1fr_auto] md:items-start" onSubmit={submit} noValidate>
          <Field label="Montant par semaine ($)" error={error}>
            {(props) => (
              <Input
                {...props}
                inputMode="numeric"
                value={amount}
                onChange={(event) => setAmount(event.target.value)}
                placeholder="40000"
              />
            )}
          </Field>
          <Field label="Ce qui compte">
            {(props) => (
              <Select value={counts} onValueChange={(value) => setCounts(value as Objective['counts'])}>
                <SelectTrigger {...props}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="total">Propre + sale</SelectItem>
                  <SelectItem value="propre">Argent propre seulement</SelectItem>
                  <SelectItem value="sale">Argent sale seulement</SelectItem>
                </SelectContent>
              </Select>
            )}
          </Field>
          <div className="flex items-center gap-2 md:pt-7">
            <Switch id="objectif-rappel" checked={reminder} onCheckedChange={setReminder} />
            <Label htmlFor="objectif-rappel">Rappel le dimanche</Label>
          </div>
          <div className="md:col-span-3">
            <Button type="submit" disabled={mutation.isPending}>
              <Save aria-hidden /> Enregistrer l'objectif
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Alertes des coffres
