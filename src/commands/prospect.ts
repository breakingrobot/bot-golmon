import { SlashCommandBuilder, ChannelType, type ChatInputCommandInteraction } from "discord.js";
import type { Command } from "../types/command.js";
import {
  handleProspectConfig,
  handleProspectListe,
  handleProspectAjouter,
  handleProspectRetirer,
  handleProspectSalon,
  handleProspectAvis,
  handleProspectHistorique,
  handleProspectReset,
  handleProspectVetoPoser,
  handleProspectVetoLever,
} from "../modules/prospects/commands/index.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("prospect")
    .setDescription("Gestion des prospects en période d'essai")
    .addSubcommand((subcommand) =>
      subcommand
        .setName("config")
        .setDescription("Configure ou affiche les paramètres du module prospect")
        .addRoleOption((option) =>
          option.setName("role_prospect").setDescription("Le rôle attribué aux membres en essai")
        )
        .addRoleOption((option) =>
          option.setName("role_grade").setDescription("Le rôle autorisé à donner son avis")
        )
        .addRoleOption((option) =>
          option.setName("role_lead").setDescription("Le rôle autorisé à poser un véto")
        )
        .addChannelOption((option) =>
          option
            .setName("categorie")
            .setDescription("La catégorie où créer les salons privés")
            .addChannelTypes(ChannelType.GuildCategory)
        )
        .addChannelOption((option) =>
          option
            .setName("salon")
            .setDescription("Le salon où publier les avis de la semaine")
            .addChannelTypes(ChannelType.GuildText)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("liste")
        .setDescription("Affiche ou reposte la liste des avis de la semaine")
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("avis")
        .setDescription("Affiche le détail complet des avis pour un membre")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le prospect ciblé").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("ajouter")
        .setDescription("Ajoute manuellement un membre à la liste de la semaine")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le membre à ajouter").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("retirer")
        .setDescription("Retire manuellement un membre de la liste de la semaine")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le membre à retirer").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("salon")
        .setDescription("Crée manuellement le salon privé d'un prospect")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le prospect ciblé").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("historique")
        .setDescription("Consulte l'historique des avis d'un membre")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le membre ciblé").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("reset")
        .setDescription("Force le démarrage d'une nouvelle session hebdomadaire")
    )
    .addSubcommandGroup((group) =>
      group
        .setName("veto")
        .setDescription("Gestion des vétos")
        .addSubcommand((subcommand) =>
          subcommand
            .setName("poser")
            .setDescription("Pose un véto sur un prospect")
            .addUserOption((option) =>
              option.setName("membre").setDescription("Le prospect ciblé").setRequired(true)
            )
            .addStringOption((option) =>
              option.setName("motif").setDescription("Motif du véto").setRequired(true)
            )
        )
        .addSubcommand((subcommand) =>
          subcommand
            .setName("lever")
            .setDescription("Lève le véto d'un prospect")
            .addUserOption((option) =>
              option.setName("membre").setDescription("Le prospect ciblé").setRequired(true)
            )
        )
    ),
  execute: async (interaction: ChatInputCommandInteraction) => {
    const group = interaction.options.getSubcommandGroup(false);
    const subcommand = interaction.options.getSubcommand();

    if (group === "veto") {
      if (subcommand === "poser") return handleProspectVetoPoser(interaction);
      if (subcommand === "lever") return handleProspectVetoLever(interaction);
    }

    if (subcommand === "config") return handleProspectConfig(interaction);
    if (subcommand === "liste") return handleProspectListe(interaction);
    if (subcommand === "ajouter") return handleProspectAjouter(interaction);
    if (subcommand === "retirer") return handleProspectRetirer(interaction);
    if (subcommand === "salon") return handleProspectSalon(interaction);
    if (subcommand === "avis") return handleProspectAvis(interaction);
    if (subcommand === "historique") return handleProspectHistorique(interaction);
    if (subcommand === "reset") return handleProspectReset(interaction);
  },
};
