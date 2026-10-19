import { Check, Copy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'

export function CopyButton({ text, label = 'Copier' }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), 1500)
    return () => window.clearTimeout(timer)
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
