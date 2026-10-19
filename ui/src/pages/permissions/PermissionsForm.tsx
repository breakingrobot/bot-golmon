import { useQueryClient } from '@tanstack/react-query'
import { Lock, RotateCcw, Save } from 'lucide-react'
import { useState } from 'react'

import { meQuery, useSavePermissions } from '@/api/queries'
import type { Level, Perm, PermissionsResponse, PermissionsUpdate } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Card, CardContent } from '@/atoms/Card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { useGuild } from '@/lib/guild-context'
import { LEVEL_LABEL, LEVELS, NO_ROLE } from '@/lib/permission-levels'
import { cn } from '@/lib/utils'
import { Field } from '@/molecules/Field'
import { PageHeader } from '@/molecules/PageHeader'
import { RoleDot } from '@/molecules/RoleDot'
import { Section } from '@/molecules/Section'
import { RoleInfo } from '@/pages/permissions/RoleInfo'

export type PermissionsFormProps = { data: PermissionsResponse }

export const PermissionsForm = ({ data }: PermissionsFormProps) => {
  const { guild } = useGuild()
  const client = useQueryClient()
  const mutation = useSavePermissions(guild.id)
  const [memberRole, setMemberRole] = useState(data.roles.member?.id ?? NO_ROLE)
  const [levels, setLevels] = useState<Partial<Record<Perm, Level>>>(() =>
    Object.fromEntries(data.actions.map((action) => [action.key, action.level])),
  )

  const grouped = new Map<string, PermissionsResponse['actions']>()
  for (const action of data.actions) grouped.set(action.module, [...(grouped.get(action.module) ?? []), action])
  const modules = [...grouped.entries()]

  // Seuls les champs modifiés partent ; revenir au niveau par défaut s'envoie comme `null`.
  const update: PermissionsUpdate = {}
  const changedLevels: Partial<Record<Perm, Level | null>> = {}
  for (const action of data.actions) {
    const next = levels[action.key] ?? action.level
    if (next !== action.level) changedLevels[action.key] = next === action.defaultLevel ? null : next
  }
  if (Object.keys(changedLevels).length > 0) update.levels = changedLevels
  const initialRole = data.roles.member?.id ?? NO_ROLE
  if (memberRole !== initialRole) update.memberRoleId = memberRole === NO_ROLE ? null : memberRole
  const dirtyCount = Object.keys(changedLevels).length + (update.memberRoleId === undefined ? 0 : 1)

  const save = () =>
    mutation.mutate(update, {
      // Les droits du compte connecté peuvent changer : recharger /api/me.
      onSuccess: () => void client.invalidateQueries({ queryKey: meQuery.queryKey }),
    })

  return (
    <>
      <PageHeader
        title="Permissions"
        description={`${guild.name} · niveau requis pour chaque action, sur Discord et sur le site`}
        actions={
          <>
            {dirtyCount > 0 && (
              <Button
                variant="ghost"
                onClick={() => {
                  setMemberRole(initialRole)
                  setLevels(Object.fromEntries(data.actions.map((action) => [action.key, action.level])))
                }}
              >
                Annuler les changements
              </Button>
            )}
            <Button onClick={save} disabled={dirtyCount === 0 || mutation.isPending}>
              <Save aria-hidden />{' '}
              {mutation.isPending ? 'Enregistrement…' : `Enregistrer${dirtyCount ? ` (${dirtyCount})` : ''}`}
            </Button>
          </>
        }
      />

      <Section title="Rôles des niveaux">
        <Card>
          <CardContent className="grid gap-4 md:grid-cols-3">
            <Field label="Rôle membre" hint="Ouvre le niveau « Membres ».">
              {(props) => (
                <Select value={memberRole} onValueChange={setMemberRole}>
                  <SelectTrigger {...props}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={NO_ROLE}>Aucun rôle</SelectItem>
                    {data.guildRoles.map((role) => (
                      <SelectItem key={role.id} value={role.id}>
                        <RoleDot color={role.color} />
                        {role.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </Field>
            <RoleInfo label="Rôle gradé" role={data.roles.grade} />
            <RoleInfo label="Rôle lead" role={data.roles.lead} />
          </CardContent>
        </Card>
        <p className="mt-2 text-xs text-ink-subtle">
          Les rôles gradé et lead se règlent sur Discord (<code className="font-mono">/config</code>). Un niveau inclut
          ceux au-dessus ; « Gérer le serveur » donne tous les niveaux, sauf pour les actions strictes.
        </p>
      </Section>

      {modules.map(([module, actions]) => (
        <Section key={module} title={module}>
          <Card className="py-0">
            <Table className="min-w-[40rem] table-fixed">
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>Action</TableHead>
                  <TableHead className="w-56">Niveau requis</TableHead>
                  <TableHead className="w-36">Par défaut</TableHead>
                  <TableHead className="w-16">
                    <span className="sr-only">Réinitialiser</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {actions.map((action) => {
                  const value = levels[action.key] ?? action.level
                  const modified = value !== action.defaultLevel
                  const changed = value !== action.level
                  return (
                    <TableRow key={action.key} className={cn(changed && 'bg-brand-soft/40 hover:bg-brand-soft/50')}>
                      <TableCell className="whitespace-normal">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="font-medium text-ink">{action.label}</span>
                          {action.strict && (
                            <Badge
                              variant="warning"
                              title="Seuls les rôles comptent : « Gérer le serveur » ne suffit pas."
                            >
                              <Lock aria-hidden /> Stricte
                            </Badge>
                          )}
                        </div>
                        <code className="font-mono text-xs text-ink-subtle">{action.key}</code>
                      </TableCell>
                      <TableCell>
                        <Select
                          value={value}
                          onValueChange={(next) => setLevels((prev) => ({ ...prev, [action.key]: next as Level }))}
                        >
                          <SelectTrigger aria-label={`Niveau requis : ${action.label}`} size="sm">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {LEVELS.map((level) => (
                              <SelectItem key={level.value} value={level.value}>
                                {level.label}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </TableCell>
                      <TableCell>
                        {modified ? (
                          <Badge variant="brand" title={`Modifié (défaut : ${LEVEL_LABEL[action.defaultLevel]})`}>
                            Modifié
                          </Badge>
                        ) : (
                          <span className="text-sm text-ink-subtle">{LEVEL_LABEL[action.defaultLevel]}</span>
                        )}
                      </TableCell>
                      <TableCell>
                        {modified && (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={`Revenir au niveau par défaut (${LEVEL_LABEL[action.defaultLevel]}) : ${action.label}`}
                            title={`Défaut : ${LEVEL_LABEL[action.defaultLevel]}`}
                            onClick={() => setLevels((prev) => ({ ...prev, [action.key]: action.defaultLevel }))}
                          >
                            <RotateCcw aria-hidden />
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </Card>
        </Section>
      ))}
    </>
  )
}
