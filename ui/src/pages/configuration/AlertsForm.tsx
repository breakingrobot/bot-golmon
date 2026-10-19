import { Plus, Save, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { useSaveCoffreAlerts } from '@/api/queries'
import type { CoffreAlerts, CoffreMeta } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Card, CardContent } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { Label } from '@/atoms/Label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { Switch } from '@/atoms/Switch'
import { propertyLabel } from '@/lib/coffre'
import { useGuild } from '@/lib/guild-context'
import { ALL } from '@/lib/search'
import { Field } from '@/molecules/Field'

/** Identifiant Discord (vide accepté : réglage retiré). */
const snowflake = (value: string) => value === '' || /^\d{15,21}$/u.test(value)

export type ThresholdRow = { key: number; propertyId: string; item: string; min: string }

/** Champs fixes, puis une erreur par seuil (`item-<clé>`, `min-<clé>`). */
type FormErrors = Partial<Record<'channel' | 'role' | 'big' | `item-${number}` | `min-${number}`, string>>

export type AlertsFormProps = { initial: CoffreAlerts; meta: CoffreMeta }

export const AlertsForm = ({ initial, meta }: AlertsFormProps) => {
  const { guild } = useGuild()
  const mutation = useSaveCoffreAlerts(guild.id)
  const [channelId, setChannelId] = useState(initial.channelId ?? '')
  const [roleId, setRoleId] = useState(initial.roleId ?? '')
  const [bigWithdrawal, setBigWithdrawal] = useState(
    initial.bigWithdrawal === null ? '' : String(initial.bigWithdrawal),
  )
  const [unknownPlayer, setUnknownPlayer] = useState(initial.unknownPlayer)
  const [rows, setRows] = useState<ThresholdRow[]>(() =>
    initial.thresholds.map((rule, index) => ({
      key: index,
      propertyId: rule.propertyId ?? ALL,
      item: rule.item,
      min: String(rule.min),
    })),
  )
  const [errors, setErrors] = useState<FormErrors>({})

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: FormErrors = {}
    if (!snowflake(channelId.trim())) next.channel = 'Identifiant Discord du salon (chiffres seulement).'
    if (!snowflake(roleId.trim())) next.role = 'Identifiant Discord du rôle (chiffres seulement).'
    const big = bigWithdrawal.trim() === '' ? null : Number(bigWithdrawal)
    if (big !== null && (!Number.isInteger(big) || big <= 0)) {
      next.big = 'Quantité entière positive, ou vide pour désactiver.'
    }
    rows.forEach((row) => {
      if (!row.item) next[`item-${row.key}`] = 'Choisissez un objet.'
      const min = Number(row.min)
      if (row.min.trim() === '' || !Number.isInteger(min) || min < 0) next[`min-${row.key}`] = 'Quantité entière.'
    })
    setErrors(next)
    if (Object.keys(next).length > 0) return
    mutation.mutate({
      channelId: channelId.trim() || null,
      roleId: roleId.trim() || null,
      bigWithdrawal: big,
      unknownPlayer,
      thresholds: rows.map((row) => ({
        propertyId: row.propertyId === ALL ? null : row.propertyId,
        item: row.item,
        min: Number(row.min),
      })),
    })
  }

  const updateRow = (key: number, patch: Partial<ThresholdRow>) =>
    setRows((prev) => prev.map((row) => (row.key === key ? { ...row, ...patch } : row)))

  return (
    <Card>
      <CardContent>
        <form className="grid gap-6" onSubmit={submit} noValidate>
          <div className="grid gap-4 md:grid-cols-2">
            <Field
              label="Salon des alertes"
              error={errors.channel}
              hint="Identifiant du salon Discord ; vide = alertes désactivées."
            >
              {(props) => (
                <Input
                  {...props}
                  value={channelId}
                  onChange={(event) => setChannelId(event.target.value)}
                  inputMode="numeric"
                  className="font-mono"
                  placeholder="1290000000000000001"
                />
              )}
            </Field>
            <Field label="Rôle mentionné" error={errors.role} hint="Identifiant du rôle ; vide = aucune mention.">
              {(props) => (
                <Input
                  {...props}
                  value={roleId}
                  onChange={(event) => setRoleId(event.target.value)}
                  inputMode="numeric"
                  className="font-mono"
                />
              )}
            </Field>
            <Field
              label="Gros retrait (quantité)"
              error={errors.big}
              hint="Un retrait unique au-dessus de cette quantité déclenche une alerte ; vide = désactivé."
            >
              {(props) => (
                <Input
                  {...props}
                  value={bigWithdrawal}
                  onChange={(event) => setBigWithdrawal(event.target.value)}
                  inputMode="numeric"
                />
              )}
            </Field>
            <div className="flex items-center gap-2 md:pt-7">
              <Switch id="alerte-inconnu" checked={unknownPlayer} onCheckedChange={setUnknownPlayer} />
              <Label htmlFor="alerte-inconnu">Alerter quand un personnage relié à aucun membre retire un objet</Label>
            </div>
          </div>

          <fieldset className="grid gap-3">
            <legend className="mb-2 text-sm font-medium text-ink">Seuils de stock bas</legend>
            {rows.length === 0 && <p className="text-sm text-ink-muted">Aucun seuil.</p>}
            {rows.map((row, index) => (
              <div
                key={row.key}
                className="grid items-start gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-[1fr_1fr_8rem_auto]"
              >
                <Field label="Propriété">
                  {(props) => (
                    <Select value={row.propertyId} onValueChange={(value) => updateRow(row.key, { propertyId: value })}>
                      <SelectTrigger {...props}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={ALL}>Toutes</SelectItem>
                        {meta.properties.map((property) => (
                          <SelectItem key={property.id} value={property.id}>
                            {propertyLabel(property)}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                </Field>
                <Field label="Objet" error={errors[`item-${row.key}`]}>
                  {(props) => (
                    <Select
                      {...(row.item ? { value: row.item } : {})}
                      onValueChange={(value) => updateRow(row.key, { item: value })}
                    >
                      <SelectTrigger {...props}>
                        <SelectValue placeholder="Choisir" />
                      </SelectTrigger>
                      <SelectContent>
                        {meta.items.map((item) => (
                          <SelectItem key={item.item} value={item.item}>
                            {item.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                </Field>
                <Field label="Minimum" error={errors[`min-${row.key}`]}>
                  {(props) => (
                    <Input
                      {...props}
                      value={row.min}
                      onChange={(event) => updateRow(row.key, { min: event.target.value })}
                      inputMode="numeric"
                    />
                  )}
                </Field>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="sm:mt-6"
                  aria-label={`Retirer le seuil ${index + 1}`}
                  onClick={() => setRows((prev) => prev.filter((item) => item.key !== row.key))}
                >
                  <Trash2 aria-hidden />
                </Button>
              </div>
            ))}
            <div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() =>
                  setRows((prev) => [
                    ...prev,
                    { key: Math.max(0, ...prev.map((row) => row.key)) + 1, propertyId: ALL, item: '', min: '' },
                  ])
                }
              >
                <Plus aria-hidden /> Ajouter un seuil
              </Button>
            </div>
          </fieldset>

          <div>
            <Button type="submit" disabled={mutation.isPending}>
              <Save aria-hidden /> Enregistrer les alertes
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  )
}
