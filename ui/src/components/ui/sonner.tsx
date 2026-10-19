import { Toaster as Sonner, type ToasterProps } from 'sonner'

import { useTheme } from '@/state/theme'

export function Toaster(props: ToasterProps) {
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
