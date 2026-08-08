import { EmbedBuilder, SlashCommandBuilder } from "discord.js";
import { RESPONSE_STATUS_LABELS } from "../../modules/presences/constants.js";
import type { Command } from "../../types/command.js";
import { computeGuildLeaderboard, computeUserStats } from "../../modules/presences/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence-stats")
    .setDescription("Statistiques de présence")
    .addUserOption((option) =>
      option.setName("joueur").setDescription("Joueur (défaut: classement du serveur)"),
    ),
  async execute(interaction) {
    if (!interaction.guildId) {
      await interaction.reply({ content: "Commande utilisable en serveur uniquement.", ephemeral: true });
      return;
    }

    const targetUser = interaction.options.getUser("joueur");

    if (targetUser) {
      const stats = await computeUserStats(interaction.guildId, targetUser.id);
      const changesText =
        stats.recentChanges.length > 0
          ? stats.recentChanges
              .map((change) => {
                const from = change.fromStatus ? RESPONSE_STATUS_LABELS[change.fromStatus] : "aucune réponse";
                const to = RESPONSE_STATUS_LABELS[change.toStatus];
                return `• ${change.response.presence.titre} : ${from} → ${to}`;
              })
              .join("\n")
          : "Aucun changement récent";

      const embed = new EmbedBuilder()
        .setTitle(`Statistiques de ${targetUser.username}`)
        .addFields(
          { name: "Présences clôturées", value: `${stats.totalClosed}`, inline: true },
          { name: "% de présence", value: `${stats.percentPresence}%`, inline: true },
          { name: "✅ Présent", value: `${stats.present}`, inline: true },
          { name: "❌ Absent", value: `${stats.absent}`, inline: true },
          { name: "⏳ Retard", value: `${stats.retard}`, inline: true },
          { name: "🤷 Sans réponse", value: `${stats.sansReponse}`, inline: true },
          { name: "Derniers changements", value: changesText },
        );

      await interaction.reply({ embeds: [embed], ephemeral: true });
      return;
    }

    const leaderboard = await computeGuildLeaderboard(interaction.guildId);
    if (leaderboard.length === 0) {
      await interaction.reply({ content: "Aucune donnée de présence pour l'instant.", ephemeral: true });
      return;
    }

    const embed = new EmbedBuilder()
      .setTitle("Classement de présence")
      .setDescription(
        leaderboard
          .slice(0, 20)
          .map(
            (entry, index) =>
              `${index + 1}. <@${entry.userId}> — ${entry.percentPresence}% (✅${entry.present} ❌${entry.absent} ⏳${entry.retard})`,
          )
          .join("\n"),
      );

    await interaction.reply({ embeds: [embed], ephemeral: true });
  },
};
