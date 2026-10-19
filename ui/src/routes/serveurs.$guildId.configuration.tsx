import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CircleCheck, CircleMinus, Plus, Save, Trash2, TriangleAlert } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import {
  coffreAlertsQuery,
  coffreMetaQuery,
  configQuery,
  objectiveQuery,
  useSaveCoffreAlerts,
  useSaveObjective,
} from '@/api/queries'
import type { CoffreAlerts, CoffreMeta, ConfigStatus, Objective } from '@/api/types'
import { propertyLabel } from '@/components/coffre-dialogs'
import { CopyButton } from '@/components/copy-button'
import { Field } from '@/components/form'
import { COUNTS_LABEL } from '@/components/objective'
import { Forbidden, PageHeader, QueryError, Section } from '@/components/page'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton, Switch } from '@/components/ui/misc'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { formatMoney } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL } from '@/lib/search'

export const Route = createFileRoute('/serveurs/$guildId/configuration')({ component: ConfigurationPage })

function ConfigurationPage() {
  const { guild } = useGuild()
  if (!guild.isAdmin) return <Forbidden>Réservé aux personnes qui peuvent « Gérer le serveur ».</Forbidden>
  return (
    <>
      <PageHeader title="Configuration" description={`${guild.name} · état des modules et réglages du site`} />
      <ModulesStatus />
      <ObjectiveSection />
      {hasPerm(guild, 'coffre.voir') && <AlertsSection />}
    </>
  )
}

const STATUS: Record<
  ConfigStatus['modules'][number]['status'],
  { label: string; variant: 'success' | 'warning' | 'neutral' }
> = {
  ok: { label: 'Prêt', variant: 'success' },
  incomplete: { label: 'Incomplet', variant: 'warning' },
  off: { label: 'Non utilisé', variant: 'neutral' },
}

function ModulesStatus() {
  const { guild } = useGuild()
  const query = useQuery(configQuery(guild.id))
  return (
    <Section
      title="État des modules"
      description="Ce qui est prêt, ce qui manque, et la commande Discord pour corriger."
    >
      {query.isPending ? (
        <div className="grid gap-4 md:grid-cols-2">
          <Skeleton className="h-36 rounded-xl" />
          <Skeleton className="h-36 rounded-xl" />
        </div>
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {query.data.modules.map((module) => {
            const status = STATUS[module.status]
            return (
              <Card key={module.key} className="gap-3">
                <CardHeader className="flex flex-row items-center justify-between gap-2">
                  <CardTitle>{module.label}</CardTitle>
                  <Badge variant={status.variant}>{status.label}</Badge>
                </CardHeader>
                <CardContent>
                  <ul className="grid gap-2">
                    {module.checks.map((check) => (
                      <li key={check.label} className="flex items-start gap-2 text-sm">
                        {check.ok ? (
                          <CircleCheck className="mt-0.5 size-4 shrink-0 text-staff" aria-label="OK" />
                        ) : module.status === 'off' ? (
                          <CircleMinus className="mt-0.5 size-4 shrink-0 text-ink-subtle" aria-label="Non utilisé" />
                        ) : (
                          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-internal" aria-label="À corriger" />
                        )}
                        <div className="min-w-0 flex-1">
                          <p className="text-ink">{check.label}</p>
                          {check.detail && <p className="text-xs text-ink-muted">{check.detail}</p>}
                          {!check.ok && check.fix && (
                            <div className="mt-1 flex items-center gap-1">
                              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-ink">
                                {check.fix}
                              </code>
                              <CopyButton text={check.fix} label="Copier la commande" />
                            </div>
                          )}
                        </div>
                      </li>
                    ))}
                  </ul>
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Objectif de cotisation

function ObjectiveSection() {
  const { guild } = useGuild()
  const query = useQuery(objectiveQuery(guild.id))
  return (
    <Section
      title="Objectif de cotisation"
      description="Montant attendu de chaque membre par semaine (lundi → dimanche, heure de Paris)."
    >
      {query.isPending ? (
        <Skeleton className="h-40 rounded-xl" />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <ObjectiveForm key={JSON.stringify(query.data)} initial={query.data} />
      )}
    </Section>
  )
}

function ObjectiveForm({ initial }: { initial: Objective | null }) {
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

function AlertsSection() {
  const { guild } = useGuild()
  const alerts = useQuery(coffreAlertsQuery(guild.id))
  const meta = useQuery(coffreMetaQuery(guild.id))
  return (
    <Section
      title="Alertes des coffres"
      description="Messages publiés sur Discord quand un stock passe sous un seuil ou qu'un retrait sort de l'ordinaire."
    >
      {alerts.isPending || meta.isPending ? (
        <Skeleton className="h-64 rounded-xl" />
      ) : alerts.isError ? (
        <QueryError error={alerts.error} retry={() => void alerts.refetch()} />
      ) : meta.isError ? (
        <QueryError error={meta.error} retry={() => void meta.refetch()} />
      ) : (
        <AlertsForm key={JSON.stringify(alerts.data)} initial={alerts.data} meta={meta.data} />
      )}
    </Section>
  )
}

type ThresholdRow = { key: number; propertyId: string; item: string; min: string }

function AlertsForm({ initial, meta }: { initial: CoffreAlerts; meta: CoffreMeta }) {
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
  const [errors, setErrors] = useState<Record<string, string>>({})

  const snowflake = (value: string) => value === '' || /^\d{15,21}$/.test(value)

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: Record<string, string> = {}
    if (!snowflake(channelId.trim())) next.channel = 'Identifiant Discord du salon (chiffres seulement).'
    if (!snowflake(roleId.trim())) next.role = 'Identifiant Discord du rôle (chiffres seulement).'
    const big = bigWithdrawal.trim() === '' ? null : Number(bigWithdrawal)
    if (big !== null && (!Number.isInteger(big) || big <= 0))
      next.big = 'Quantité entière positive, ou vide pour désactiver.'
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
