// Dialogue « Enregistrer un véhicule » : plaque, propriétaire (membre du groupe ou rôle), modèle facultatif.

import { Plus } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useCreateVehicle } from '@/api/queries-groupe'
import type { MemberRef, Role, VehicleInput } from '@/api/types'
import { Button } from '@/atoms/Button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/atoms/Dialog'
import { Input } from '@/atoms/Input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { Field } from '@/molecules/Field'
import { MemberPicker } from '@/molecules/MemberPicker'
import { RoleDot } from '@/molecules/RoleDot'

/** Plaque comme le bot : majuscules, sans espace ni tiret. */
export const normalizePlate = (text: string): string => text.trim().toUpperCase().replaceAll(/[\s-]/gu, '')

type FormErrors = Partial<Record<'plate' | 'owner' | 'model', string>>

export type AddVehicleDialogProps = { guildId: string; roles: Role[] }

export const AddVehicleDialog = ({ guildId, roles }: AddVehicleDialogProps) => {
  const [open, setOpen] = useState(false)
  const [plate, setPlate] = useState('')
  const [ownerKind, setOwnerKind] = useState<'membre' | 'role'>('membre')
  const [member, setMember] = useState<MemberRef | null>(null)
  const [roleId, setRoleId] = useState('')
  const [model, setModel] = useState('')
  const [errors, setErrors] = useState<FormErrors>({})
  const mutation = useCreateVehicle(guildId)

  const reset = () => {
    setPlate('')
    setOwnerKind('membre')
    setMember(null)
    setRoleId('')
    setModel('')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: FormErrors = {}
    const normalized = normalizePlate(plate)
    if (!/^[A-Z0-9]{1,12}$/u.test(normalized)) next.plate = 'Lettres et chiffres, 12 au plus.'
    const ownerId = ownerKind === 'membre' ? member?.id : roleId
    if (!ownerId) next.owner = ownerKind === 'membre' ? 'Choisissez le membre.' : 'Choisissez le rôle.'
    if (model.trim().length > 60) next.model = '60 caractères au plus.'
    setErrors(next)
    if (Object.keys(next).length > 0 || !ownerId) return
    const body: VehicleInput = {
      plate: normalized,
      ownerId,
      ownerIsRole: ownerKind === 'role',
      ...(model.trim() ? { model: model.trim() } : {}),
    }
    mutation.mutate(body, {
      onSuccess: () => {
        setOpen(false)
        reset()
      },
    })
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) reset()
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus aria-hidden /> Enregistrer un véhicule
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Enregistrer un véhicule</DialogTitle>
          <DialogDescription>
            Sans modèle, il est demandé à l'API du jeu (une panne de l'API n'empêche pas l'ajout). Le tableau de bord
            Discord est mis à jour.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <Field label="Plaque" error={errors.plate}>
            {(props) => (
              <Input
                {...props}
                value={plate}
                onChange={(event) => setPlate(event.target.value)}
                placeholder="D6CS3P0G"
                className="font-mono uppercase"
                autoComplete="off"
                autoFocus
              />
            )}
          </Field>
          <div className="grid gap-2">
            <span className="text-sm font-medium text-ink-muted" id="vehicule-proprietaire">
              Propriétaire
            </span>
            <ToggleGroup
              type="single"
              value={ownerKind}
              onValueChange={(value) => value && setOwnerKind(value as 'membre' | 'role')}
              aria-labelledby="vehicule-proprietaire"
            >
              <ToggleGroupItem value="membre">Un membre</ToggleGroupItem>
              <ToggleGroupItem value="role">Un rôle</ToggleGroupItem>
            </ToggleGroup>
            {ownerKind === 'membre' ? (
              <Field label="Membre" error={errors.owner}>
                {(props) => (
                  <MemberPicker
                    guildId={guildId}
                    value={member}
                    onChange={setMember}
                    id={props.id}
                    invalid={Boolean(errors.owner)}
                  />
                )}
              </Field>
            ) : (
              <Field label="Rôle" error={errors.owner}>
                {(props) => (
                  <Select value={roleId} onValueChange={setRoleId}>
                    <SelectTrigger {...props}>
                      <SelectValue placeholder="Choisir un rôle" />
                    </SelectTrigger>
                    <SelectContent>
                      {roles.map((role) => (
                        <SelectItem key={role.id} value={role.id}>
                          <RoleDot color={role.color} /> {role.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </Field>
            )}
          </div>
          <Field label="Modèle" error={errors.model} hint="Facultatif.">
            {(props) => (
              <Input {...props} value={model} onChange={(event) => setModel(event.target.value)} placeholder="Sultan" />
            )}
          </Field>
          {mutation.isError && (
            <p role="alert" className="rounded-md border border-danger/40 bg-danger-soft px-3 py-2 text-sm text-danger">
              {errorMessage(mutation.error)}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? 'Enregistrement…' : 'Enregistrer'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
