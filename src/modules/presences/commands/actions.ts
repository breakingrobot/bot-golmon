import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import type { ResponseStatus } from "@prisma/client";
import { chunkMentions, truncateField } from "../../../lib/format.js";
import { formatDateLabel } from "../../../lib/datetime.js";
import { RESPONSE_STATUS_EMOJIS, RESPONSE_STATUS_LABELS } from "../constants.js";
import { logger } from "../../../lib/logger.js";
import {
  closePresence,
  findAllPresences,
  findOpenPresences,
  getCurrentPresence,
  getNonRespondentIds,
  getPresenceWithResponses,
  registerResponse,
  resetGuildPresences,
  syncPresenceMessage,
  deletePresence,
} from "../service.js";

export async function autocompletePresence(interaction: AutocompleteInteraction) {
  if (!interaction.guildId) {
    await interaction.respond([]);
    return;
  }

  const query = interaction.options.getFocused();
  // forcevote sert aussi à corriger après coup : il doit voir les clôturées.
  const presences =
    interaction.options.getSubcommand() === "forcevote"
      ? await findAllPresences(interaction.guildId, query)
      : await findOpenPresences(interaction.guildId, query);

  await interaction.respond(
    presences.map((presence) => ({
      name: `${presence.closedAt ? "🔒 " : ""}${presence.titre} — ${formatDateLabel(presence.eventDate)}`.slice(0, 100),
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

export async function handlePresenceCancel(interaction: ChatInputCommandInteraction) {
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Tu dois avoir la permission de gérer les messages pour annuler une présence.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const presenceId = interaction.options.getString("presence", true);
  const presence = await getPresenceWithResponses(presenceId);

  if (!presence) {
    await interaction.reply({
      content: "Présence introuvable.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  if (presence.messageId) {
    const channel = await interaction.client.channels.fetch(presence.channelId).catch(() => null);
    if (channel?.isSendable()) {
      await channel.messages.delete(presence.messageId).catch(() => null);
    }
  }

  await deletePresence(presenceId);
  await interaction.editReply("Présence annulée et supprimée.");
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

  const channel = await interaction.client.channels.fetch(presence.channelId).catch((err) => {
    logger.warn(`Impossible de récupérer le salon de la présence : ${err.message}`);
    return null;
  });
  if (!channel?.isSendable()) {
    await interaction.editReply("Salon de la présence inaccessible.");
    return;
  }

  for (const content of chunkMentions(nonRespondentIds, `⏰ Rappel — **${presence.titre}**`)) {
    await channel.send({ content, allowedMentions: { parse: ["users"] } });
  }

  await interaction.editReply(`${nonRespondentIds.length} membre(s) relancé(s).`);
}

export async function handlePresenceForceVote(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Réservé aux membres pouvant gérer les messages.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const target = interaction.options.getUser("utilisateur", true);
  const statut = interaction.options.getString("statut", true) as ResponseStatus;
  const retard = interaction.options.getInteger("retard") ?? undefined;

  // Sans présence précisée, on vise la présence ouverte la plus proche.
  const presenceId = interaction.options.getString("presence");
  const presence = presenceId
    ? await getPresenceWithResponses(presenceId)
    : await getCurrentPresence(interaction.guildId);

  if (!presence) {
    await interaction.reply({
      content: "Aucune présence trouvée. Précise-la avec l'option `presence`.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const { changed } = await registerResponse(presence.id, target.id, statut, retard);
  if (changed) {
    await syncPresenceMessage(interaction.client, presence.id);
  }

  const label = `${RESPONSE_STATUS_EMOJIS[statut]} ${RESPONSE_STATUS_LABELS[statut]}`;
  const suffix = changed ? "" : " (déjà le cas, rien changé)";
  await interaction.editReply(
    `${target} défini sur **${label}** pour « ${presence.titre} »${suffix}.`,
  );
}

export async function handlePresenceChanges(interaction: ChatInputCommandInteraction) {
  const presenceId = interaction.options.getString("presence", true);
  const presence = await getPresenceWithResponses(presenceId);

  if (!presence) {
    await interaction.reply({ content: "Présence introuvable.", flags: MessageFlags.Ephemeral });
    return;
  }

  const responsesWithChanges = presence.responses.filter((r) =>
    r.changes.some((c) => c.fromStatus !== null),
  );

  if (responsesWithChanges.length === 0) {
    await interaction.reply({
      content: "Aucun changement d'avis pour cette présence.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const changesText = responsesWithChanges
    .map((r) => {
      const changes = [...r.changes]
        .filter((c) => c.fromStatus !== null)
        .sort((a, b) => b.changedAt.getTime() - a.changedAt.getTime());

      const changesList = changes
        .map((c) => {
          const from = `${RESPONSE_STATUS_EMOJIS[c.fromStatus!]} ${RESPONSE_STATUS_LABELS[c.fromStatus!]}`;
          const to = `${RESPONSE_STATUS_EMOJIS[c.toStatus]} ${RESPONSE_STATUS_LABELS[c.toStatus]}`;
          return `${from} → ${to} (<t:${Math.floor(c.changedAt.getTime() / 1000)}:R>)`;
        })
        .join(", ");

      return `• <@${r.userId}> : ${changesList}`;
    })
    .join("\n");

  const embed = new EmbedBuilder()
    .setTitle(`Changements d'avis - ${presence.titre}`)
    .setColor(0x5865f2)
    .setDescription(truncateField(changesText, 4096));

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}

export async function handlePresenceReset(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent réinitialiser les statistiques.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await resetGuildPresences(interaction.guildId);
  await interaction.editReply("Toutes les statistiques et historiques de présences ont été réinitialisés avec succès.");
}
