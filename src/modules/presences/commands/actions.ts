import {
  MessageFlags,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import { chunkMentions } from "../../../lib/format.js";
import {
  closePresence,
  findOpenPresences,
  getNonRespondentIds,
  getPresenceWithResponses,
  syncPresenceMessage,
} from "../service.js";

export async function autocompletePresence(interaction: AutocompleteInteraction) {
  if (!interaction.guildId) {
    await interaction.respond([]);
    return;
  }
  const presences = await findOpenPresences(interaction.guildId, interaction.options.getFocused());
  await interaction.respond(
    presences.map((presence) => ({
      name: presence.titre.slice(0, 80),
      value: presence.id,
    })),
  );
}

export async function handlePresenceClose(interaction: ChatInputCommandInteraction) {
  const presenceId = interaction.options.getString("presence", true);
  const presence = await getPresenceWithResponses(presenceId);

  if (!presence) {
    await interaction.reply({
      content: "Présence introuvable.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await closePresence(presenceId);
  await syncPresenceMessage(interaction.client, presenceId);
  await interaction.reply({ content: "Présence clôturée.", flags: MessageFlags.Ephemeral });
}

export async function handlePresencePing(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const presenceId = interaction.options.getString("presence", true);
  const presence = await getPresenceWithResponses(presenceId);
  if (!presence) {
    await interaction.reply({ content: "Présence introuvable.", flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const nonRespondentIds = await getNonRespondentIds(interaction.guild, presence);
  if (nonRespondentIds.length === 0) {
    await interaction.editReply("Tout le monde a répondu.");
    return;
  }

  const channel = await interaction.client.channels.fetch(presence.channelId).catch(() => null);
  if (!channel?.isSendable()) {
    await interaction.editReply("Salon de la présence inaccessible.");
    return;
  }

  for (const content of chunkMentions(nonRespondentIds, `⏰ Rappel — **${presence.titre}**`)) {
    await channel.send({ content, allowedMentions: { parse: ["users"] } });
  }

  await interaction.editReply(`${nonRespondentIds.length} membre(s) relancé(s).`);
}
