import { useLocation } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { loginHref } from '@/api/client'

export type LoginLinkProps = { loginUrl: string } & Omit<ComponentProps<'a'>, 'href'>

/** Lien « Se connecter » (navigation complète vers le bot) qui ramène à la page affichée. */
export const LoginLink = ({ loginUrl, children, ...props }: LoginLinkProps) => {
  // Abonnement à l'adresse : le paramètre `next` suit la navigation.
  useLocation({ select: (location) => location.href })
  return (
    <a href={loginHref(loginUrl)} {...props}>
      {children}
    </a>
  )
}
