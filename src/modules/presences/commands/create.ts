import { MessageFlags, type ChatInputCommandInteraction } from "discord.js";
import { resolveEventDate } from "../../../lib/datetime.js";
import { chunkMentions } from "../../../lib/format.js";
import { getRoleMemberIds } from "../../../lib/memberCache.js";
import { logger } from "../../../lib/logger.js";
import {
  createPresence,
  deletePresence,
  getPresenceConfig,
  syncPresenceMessage,
} from "../service.js";

export async function handlePresenceCreate(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const config = await getPresenceConfig(interaction.guild.id);
  if (!config?.defaultRoleId) {
    await interaction.reply({
      content:
        "Aucun rôle configuré. Lance d'abord `/presence config role:@TonRôle` (une seule fois).",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const heure = interaction.options.getString("heure", true);
  const dateOption = interaction.options.getString("date");
  const resolved = resolveEventDate(dateOption, heure);

  if (!resolved) {
    await interaction.reply({
      content: "Date ou heure invalide. Formats attendus : `JJ/MM/AAAA` et `HH:MM`.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const channelId = config.presenceChannelId ?? interaction.channelId;
  const channel = await interaction.client.channels.fetch(channelId).catch(() => null);
  if (!channel?.isSendable()) {
    await interaction.reply({
      content: "Salon des présences introuvable ou inaccessible. Vérifie `/presence config`.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const presence = await createPresence(interaction.guild, {
    channelId,
    titre: interaction.options.getString("titre") ?? "Présence",
    description: interaction.options.getString("description") ?? undefined,
    eventDate: resolved.date,
    roleId: config.defaultRoleId,
    exceptionRoleId: config.exceptionRoleId ?? undefined,
    createdById: interaction.user.id,
  });

  try {
    await syncPresenceMessage(interaction.client, presence.id);
  } catch (error) {
    // Sans message publié, la présence serait invisible mais génèrerait des
    // rappels : on retire la ligne plutôt que de laisser un fantôme en base.
    logger.error(`Publication de la présence ${presence.id} échouée, suppression`, error);
    await deletePresence(presence.id).catch(() => null);
    await interaction.editReply({
      content: "La présence n'a pas pu être publiée (Discord a refusé la requête). Réessaie dans un instant.",
    });
    return;
  }

  // Ping initial : membres du rôle, hors rôle d'exception (déjà en absent).
  const targetIds = await getRoleMemberIds(interaction.guild, config.defaultRoleId);
  const excludedIds = config.exceptionRoleId
    ? new Set(await getRoleMemberIds(interaction.guild, config.exceptionRoleId))
    : new Set<string>();
  const mentionable = targetIds.filter((id) => !excludedIds.has(id));

  for (const content of chunkMentions(mentionable, "📣")) {
    await channel.send({ content, allowedMentions: { parse: ["users"] } });
  }

  const rolled = resolved.rolledToTomorrow ? " (heure déjà passée aujourd'hui → **demain**)" : "";
  await interaction.editReply({
    content: `Présence créée pour <t:${Math.floor(resolved.date.getTime() / 1000)}:F>${rolled} dans <#${channelId}>.`,
  });
}
