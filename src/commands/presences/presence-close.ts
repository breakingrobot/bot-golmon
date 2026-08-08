import { SlashCommandBuilder } from "discord.js";
import type { Command } from "../../types/command.js";
import { closePresence, findOpenPresences, syncPresenceMessage } from "../../modules/presences/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence-close")
    .setDescription("Clôture une présence")
    .addStringOption((option) =>
      option
        .setName("presence_id")
        .setDescription("Présence concernée")
        .setRequired(true)
        .setAutocomplete(true),
    ),
  async execute(interaction) {
    const presenceId = interaction.options.getString("presence_id", true);
    await closePresence(presenceId);
    await syncPresenceMessage(interaction.client, presenceId);
    await interaction.reply({ content: "Présence clôturée.", ephemeral: true });
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
