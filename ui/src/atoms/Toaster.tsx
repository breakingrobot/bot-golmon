import { Toaster as Sonner } from 'sonner'
import type { ToasterProps } from 'sonner'

import { useTheme } from '@/state/theme'

export const Toaster = (props: ToasterProps) => {
  const { resolved } = useTheme()
  return (
    <Sonner
      theme={resolved}
      position="bottom-right"
      // Au-dessus de la barre d'onglets du bas sur téléphone.
      mobileOffset={{ bottom: 'calc(4.5rem + env(safe-area-inset-bottom))' }}
      closeButton
      richColors
      toastOptions={{ classNames: { toast: 'font-sans' } }}
      {...props}
    />
  )
}
