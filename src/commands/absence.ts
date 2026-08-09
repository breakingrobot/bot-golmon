import { SlashCommandBuilder } from "discord.js";
import type { Command } from "../types/command.js";
import {
  handleAbsenceDeclare,
  handleAbsenceEnd,
  handleAbsenceList,
} from "../modules/presences/commands/absence.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("absence")
    .setDescription("Signaler une absence")
    .addSubcommand((sub) =>
      sub
        .setName("declare")
        .setDescription("Signale ton absence (le rôle d'absence est appliqué automatiquement)")
        .addStringOption((option) =>
          option.setName("motif").setDescription("Motif de l'absence").setRequired(true),
        )
        .addStringOption((option) =>
          option.setName("fin").setDescription("Dernier jour d'absence, JJ/MM/AAAA").setRequired(true),
        )
        .addStringOption((option) =>
          option.setName("debut").setDescription("Premier jour, JJ/MM/AAAA (défaut: aujourd'hui)"),
        )
        .addUserOption((option) =>
          option.setName("membre").setDescription("Déclarer pour un autre membre (staff)"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("end")
        .setDescription("Met fin à une absence en cours")
        .addUserOption((option) =>
          option.setName("membre").setDescription("Membre concerné (staff)"),
        ),
    )
    .addSubcommand((sub) =>
      sub.setName("list").setDescription("Liste les absences en cours"),
    ),

  async execute(interaction) {
    switch (interaction.options.getSubcommand()) {
      case "declare":
        return handleAbsenceDeclare(interaction);
      case "end":
        return handleAbsenceEnd(interaction);
      case "list":
        return handleAbsenceList(interaction);
      default:
        return;
    }
  },
};
