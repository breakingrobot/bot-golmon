import { Toaster as Sonner } from 'sonner'
import type { ToasterProps } from 'sonner'

import { useTheme } from '@/state/theme'

export const Toaster = (props: ToasterProps) => {
  const { resolved } = useTheme()
  return (
    <Sonner
      theme={resolved}
      position="bottom-right"
      closeButton
      richColors
      toastOptions={{ classNames: { toast: 'font-sans' } }}
      {...props}
    />
  )
}
