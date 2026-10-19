import { Check, Copy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'

import { Button } from '@/atoms/Button'

export type CopyButtonProps = { text: string; label?: string }

export const CopyButton = ({ text, label = 'Copier' }: CopyButtonProps) => {
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!copied) return
    const timer = globalThis.setTimeout(() => setCopied(false), 1500)
    return () => globalThis.clearTimeout(timer)
  }, [copied])

  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={`${label} : ${text}`}
      title={label}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text)
          setCopied(true)
        } catch {
          toast.error('Copie impossible : sélectionnez le texte à la main.')
        }
      }}
    >
      {copied ? <Check className="text-staff" aria-hidden /> : <Copy aria-hidden />}
    </Button>
  )
}
