import { StickyNote } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { useAddContactNote } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { Textarea } from '@/atoms/Textarea'
import { LIMITS } from '@/lib/contact'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

export type AddNoteProps = { guildId: string; contactId: string }

export const AddNote = ({ guildId, contactId }: AddNoteProps) => {
  const [body, setBody] = useState('')
  const [error, setError] = useState<string | null>(null)
  const mutation = useAddContactNote(guildId)
  const submit = (event: FormEvent) => {
    event.preventDefault()
    const text = body.trim()
    if (!text) return setError('La note est vide.')
    if (text.length > LIMITS.note) return setError(`${LIMITS.note} caractères au plus.`)
    setError(null)
    mutation.mutate({ id: contactId, body: text }, { onSuccess: () => setBody('') })
  }
  return (
    <form className="mt-3 grid gap-2" onSubmit={submit} noValidate>
      <Field label="Nouvelle note" error={error} hint={<CharCount value={body} max={LIMITS.note} />}>
        {(props) => (
          <Textarea
            {...props}
            value={body}
            onChange={(event) => setBody(event.target.value)}
            rows={3}
            placeholder="Visible par le staff seulement"
          />
        )}
      </Field>
      <div>
        <Button type="submit" size="sm" disabled={mutation.isPending}>
          <StickyNote aria-hidden /> Ajouter la note
        </Button>
      </div>
    </form>
  )
}
