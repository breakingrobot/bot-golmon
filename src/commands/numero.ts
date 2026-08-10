import { SlashCommandBuilder, ChannelType, type ChatInputCommandInteraction } from "discord.js";
import type { Command } from "../types/command.js";
import {
  handlePhoneConfig,
  handlePhoneAjouter,
  handlePhoneVoir,
  handlePhoneSupprimer,
  handlePhoneListe,
} from "../modules/phones/commands/index.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("numero")
    .setDescription("Gestion de l'annuaire téléphonique")
    .addSubcommand((subcommand) =>
      subcommand
        .setName("config")
        .setDescription("Configure le salon d'affichage de l'annuaire")
        .addChannelOption((option) =>
          option
            .setName("salon")
            .setDescription("Le salon où publier l'annuaire")
            .addChannelTypes(ChannelType.GuildText)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("ajouter")
        .setDescription("Enregistre ton numéro de téléphone")
        .addStringOption((option) =>
          option.setName("numero").setDescription("Ton numéro (ex: 5551234)").setRequired(true)
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("voir")
        .setDescription("Consulte le numéro d'un membre")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le membre ciblé (laisser vide pour soi-même)")
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("supprimer")
        .setDescription("Supprime un numéro de téléphone")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Le membre ciblé (laisser vide pour soi-même, gradé requis pour les autres)")
        )
    )
    .addSubcommand((subcommand) =>
      subcommand
        .setName("liste")
        .setDescription("Republie la liste des numéros")
    ),
  execute: async (interaction: ChatInputCommandInteraction) => {
    const subcommand = interaction.options.getSubcommand();
    if (subcommand === "config") return handlePhoneConfig(interaction);
    if (subcommand === "ajouter") return handlePhoneAjouter(interaction);
    if (subcommand === "voir") return handlePhoneVoir(interaction);
    if (subcommand === "supprimer") return handlePhoneSupprimer(interaction);
    if (subcommand === "liste") return handlePhoneListe(interaction);
  },
};
