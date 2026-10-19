// Espace d'un serveur : barre latérale (tiroir sur mobile), sélecteur de serveur, pages autorisées.

import { createFileRoute } from '@tanstack/react-router'

import { GuildLayout } from '@/templates/GuildLayout'

export const Route = createFileRoute('/serveurs/$guildId')({ component: GuildLayout })
