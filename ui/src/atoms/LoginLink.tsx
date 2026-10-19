import { useLocation } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { loginHref } from '@/api/client'

/** Lien « Se connecter » (navigation complète vers le bot) qui ramène à la page affichée. */
export function LoginLink({ loginUrl, children, ...props }: { loginUrl: string } & Omit<ComponentProps<'a'>, 'href'>) {
  // Abonnement à l'adresse : le paramètre `next` suit la navigation.
  useLocation({ select: (location) => location.href })
  return (
    <a href={loginHref(loginUrl)} {...props}>
      {children}
    </a>
  )
}
