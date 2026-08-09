import { EmbedBuilder, MessageFlags, type ChatInputCommandInteraction } from "discord.js";
import { RESPONSE_STATUS_EMOJIS, RESPONSE_STATUS_LABELS } from "../constants.js";
import { truncateField } from "../../../lib/format.js";
import { computeGuildLeaderboard, computeUserStats } from "../service.js";

export async function handlePresenceStats(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const targetUser = interaction.options.getUser("joueur");

  if (targetUser) {
    const stats = await computeUserStats(interaction.guildId, targetUser.id);
    const changesText =
      stats.recentChanges.length > 0
        ? stats.recentChanges
            .map((change) => {
              const from = change.fromStatus
                ? `${RESPONSE_STATUS_EMOJIS[change.fromStatus]} ${RESPONSE_STATUS_LABELS[change.fromStatus]}`
                : "*aucune réponse*";
              const to = `${RESPONSE_STATUS_EMOJIS[change.toStatus]} ${RESPONSE_STATUS_LABELS[change.toStatus]}`;
              return `• **${change.response.presence.titre}** : ${from} → ${to}`;
            })
            .join("\n")
        : "Aucun changement";

    const embed = new EmbedBuilder()
      .setTitle(`Statistiques de ${targetUser.username}`)
      .setThumbnail(targetUser.displayAvatarURL())
      .setColor(0x5865f2)
      .addFields(
        { name: "Présences clôturées", value: `${stats.totalClosed}`, inline: true },
        { name: "% de présence", value: `${stats.percentPresence}%`, inline: true },
        { name: "​", value: "​", inline: true },
        { name: "✅ Présent", value: `${stats.present}`, inline: true },
        { name: "❌ Absent", value: `${stats.absent}`, inline: true },
        { name: "⏳ Retard", value: `${stats.retard}`, inline: true },
        { name: "🕓 Sans réponse", value: `${stats.sansReponse}`, inline: true },
        { name: "Derniers changements d'avis", value: truncateField(changesText) },
      );

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  const leaderboard = await computeGuildLeaderboard(interaction.guildId);
  if (leaderboard.length === 0) {
    await interaction.reply({
      content: "Aucune donnée de présence pour l'instant.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const embed = new EmbedBuilder()
    .setTitle("Classement de présence")
    .setColor(0x5865f2)
    .setDescription(
      truncateField(
        leaderboard
          .slice(0, 20)
          .map(
            (entry, index) =>
              `${index + 1}. <@${entry.userId}> — **${entry.percentPresence}%** (✅${entry.present} ❌${entry.absent} ⏳${entry.retard})`,
          )
          .join("\n"),
        4096,
      ),
    );

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}
