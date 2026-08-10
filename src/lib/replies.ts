import { MessageFlags } from "discord.js";

/** Réponse partagée par toutes les commandes inutilisables en message privé. */
export const GUILD_ONLY = {
  content: "Commande utilisable en serveur uniquement.",
  flags: MessageFlags.Ephemeral,
} as const;
