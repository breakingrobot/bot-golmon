import { Download } from 'lucide-react'

import { Button } from '@/atoms/Button'

export function CsvButton({ href }: { href: string }) {
  return (
    <Button asChild variant="outline" size="sm">
      <a href={href} download>
        <Download aria-hidden /> Exporter en CSV
      </a>
    </Button>
  )
}
