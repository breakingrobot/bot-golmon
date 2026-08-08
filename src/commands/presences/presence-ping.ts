import { SlashCommandBuilder } from "discord.js";
import type { Command } from "../../types/command.js";
import { findOpenPresences, getNonRespondentIds, getPresenceWithResponses } from "../../modules/presences/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence-ping")
    .setDescription("Re-ping manuellement les non-répondants d'une présence")
    .addStringOption((option) =>
      option
        .setName("presence_id")
        .setDescription("Présence concernée")
        .setRequired(true)
        .setAutocomplete(true),
    ),
  async execute(interaction) {
    if (!interaction.guild) {
      await interaction.reply({ content: "Commande utilisable en serveur uniquement.", ephemeral: true });
      return;
    }

    const presenceId = interaction.options.getString("presence_id", true);
    const presence = await getPresenceWithResponses(presenceId);
    if (!presence) {
      await interaction.reply({ content: "Présence introuvable.", ephemeral: true });
      return;
    }

    const nonRespondentIds = await getNonRespondentIds(interaction.guild, presence);
    if (nonRespondentIds.length === 0) {
      await interaction.reply({ content: "Tout le monde a répondu.", ephemeral: true });
      return;
    }

    await interaction.reply({
      content: `Rappel présence : ${nonRespondentIds.map((userId) => `<@${userId}>`).join(" ")}`,
      allowedMentions: { parse: ["users"] },
    });
  },
  async autocomplete(interaction) {
    if (!interaction.guildId) {
      await interaction.respond([]);
      return;
    }
    const query = interaction.options.getFocused();
    const presences = await findOpenPresences(interaction.guildId, query);
    await interaction.respond(presences.map((p) => ({ name: `${p.titre} — ${p.id}`, value: p.id })));
  },
};
