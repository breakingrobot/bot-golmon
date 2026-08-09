import { ChannelType, SlashCommandBuilder } from "discord.js";
import type { Command } from "../types/command.js";
import { handlePresenceConfig } from "../modules/presences/commands/config.js";
import { handlePresenceCreate } from "../modules/presences/commands/create.js";
import { handlePresenceStats } from "../modules/presences/commands/stats.js";
import { RESPONSE_STATUS_CHOICES } from "../modules/presences/constants.js";
import {
  autocompletePresence,
  handlePresenceClose,
  handlePresenceForceVote,
  handlePresencePing,
} from "../modules/presences/commands/actions.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence")
    .setDescription("Gestion des présences de groupe")
    .addSubcommand((sub) =>
      sub
        .setName("create")
        .setDescription("Crée une présence (rôles et salon viennent de la config)")
        .addStringOption((option) =>
          option.setName("heure").setDescription("Heure, ex: 21:00").setRequired(true),
        )
        .addStringOption((option) =>
          option.setName("titre").setDescription("Titre (défaut: Présence)"),
        )
        .addStringOption((option) =>
          option.setName("date").setDescription("Date JJ/MM/AAAA (défaut: aujourd'hui)"),
        )
        .addStringOption((option) => option.setName("description").setDescription("Description")),
    )
    .addSubcommand((sub) =>
      sub
        .setName("config")
        .setDescription("Configure le module (sans option: affiche la config actuelle)")
        .addRoleOption((option) => option.setName("role").setDescription("Rôle ciblé par les présences"))
        .addRoleOption((option) =>
          option.setName("exception_role").setDescription("Rôle d'exception (auto-absent, non ping)"),
        )
        .addChannelOption((option) =>
          option
            .setName("channel")
            .setDescription("Salon où poster les présences")
            .addChannelTypes(ChannelType.GuildText),
        )
        .addChannelOption((option) =>
          option
            .setName("reminder_channel")
            .setDescription("Salon des rappels automatiques")
            .addChannelTypes(ChannelType.GuildText),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("stats")
        .setDescription("Statistiques de présence")
        .addUserOption((option) =>
          option.setName("joueur").setDescription("Joueur (défaut: classement du serveur)"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("ping")
        .setDescription("Relance les membres n'ayant pas répondu")
        .addStringOption((option) =>
          option
            .setName("presence")
            .setDescription("Présence concernée")
            .setRequired(true)
            .setAutocomplete(true),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("forcevote")
        .setDescription("Définir manuellement le vote d'un membre (staff)")
        .addUserOption((option) =>
          option.setName("utilisateur").setDescription("Membre concerné").setRequired(true),
        )
        .addStringOption((option) =>
          option
            .setName("statut")
            .setDescription("Statut à appliquer")
            .setRequired(true)
            .addChoices(...RESPONSE_STATUS_CHOICES),
        )
        .addStringOption((option) =>
          option
            .setName("presence")
            .setDescription("Présence visée (défaut: la plus proche)")
            .setAutocomplete(true),
        )
        .addIntegerOption((option) =>
          option
            .setName("retard")
            .setDescription("Durée du retard en minutes")
            .setMinValue(0),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("close")
        .setDescription("Clôture une présence")
        .addStringOption((option) =>
          option
            .setName("presence")
            .setDescription("Présence concernée")
            .setRequired(true)
            .setAutocomplete(true),
        ),
    ),

  async execute(interaction) {
    switch (interaction.options.getSubcommand()) {
      case "create":
        return handlePresenceCreate(interaction);
      case "config":
        return handlePresenceConfig(interaction);
      case "stats":
        return handlePresenceStats(interaction);
      case "ping":
        return handlePresencePing(interaction);
      case "forcevote":
        return handlePresenceForceVote(interaction);
      case "close":
        return handlePresenceClose(interaction);
      default:
        return;
    }
  },

  async autocomplete(interaction) {
    return autocompletePresence(interaction);
  },
};
