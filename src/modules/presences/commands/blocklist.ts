import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type ChatInputCommandInteraction,
} from "discord.js";
import { truncateField } from "../../../lib/format.js";
import { addToBlocklist, listBlocklist, removeFromBlocklist } from "../blocklist.js";

export async function handlePresenceBlocklist(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const sub = interaction.options.getSubcommand();

  if (sub === "list") {
    const entries = await listBlocklist(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("🚫 Blocklist présences")
      .setColor(0x95a5a6)
      .setDescription(
        entries.length === 0
          ? "Personne dans la blocklist."
          : truncateField(
              entries
                .map((entry) => `• <@${entry.userId}>${entry.raison ? ` — ${entry.raison}` : ""}`)
                .join("\n"),
              4096,
            ),
      )
      .setFooter({ text: `${entries.length} membre(s) jamais pingé(s)` });

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  // L'ajout et le retrait modifient qui reçoit les pings : réservés au staff.
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Réservé aux membres pouvant gérer les messages.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const member = interaction.options.getUser("membre", true);

  if (sub === "add") {
    await addToBlocklist({
      guildId: interaction.guildId,
      userId: member.id,
      raison: interaction.options.getString("raison") ?? undefined,
      addedById: interaction.user.id,
    });
    await interaction.reply({
      content: `${member} ne sera plus pingé pour les présences et sera marqué absent d'office.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (sub === "remove") {
    const removed = await removeFromBlocklist(interaction.guildId, member.id);
    await interaction.reply({
      content: removed
        ? `${member} retiré de la blocklist.`
        : `${member} n'était pas dans la blocklist.`,
      flags: MessageFlags.Ephemeral,
    });
  }
}
