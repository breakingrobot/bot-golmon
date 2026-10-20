import { Download } from 'lucide-react'

import { Button } from '@/atoms/Button'

export type CsvButtonProps = { href: string }

export const CsvButton = ({ href }: CsvButtonProps) => (
  <Button asChild variant="outline" size="sm">
    <a href={href} download>
      <Download aria-hidden /> Exporter en CSV
    </a>
  </Button>
)
