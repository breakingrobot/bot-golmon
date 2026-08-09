import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type ChatInputCommandInteraction,
} from "discord.js";
import { getPresenceConfig, upsertPresenceConfig } from "../service.js";

function describe(id: string | null | undefined, kind: "role" | "channel"): string {
  if (!id) {
    return "*non configuré*";
  }
  return kind === "role" ? `<@&${id}>` : `<#${id}>`;
}

export async function handlePresenceConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply({
      content: "Commande utilisable en serveur uniquement.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const role = interaction.options.getRole("role");
  const exceptionRole = interaction.options.getRole("exception_role");
  const presenceChannel = interaction.options.getChannel("channel");
  const reminderChannel = interaction.options.getChannel("reminder_channel");

  // Sans aucune option, la commande sert à consulter la configuration.
  if (!role && !exceptionRole && !presenceChannel && !reminderChannel) {
    const config = await getPresenceConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration des présences")
      .setColor(0x5865f2)
      .addFields(
        { name: "Rôle ciblé", value: describe(config?.defaultRoleId, "role") },
        { name: "Rôle d'exception", value: describe(config?.exceptionRoleId, "role") },
        { name: "Salon des présences", value: describe(config?.presenceChannelId, "channel") },
        { name: "Salon des rappels", value: describe(config?.reminderChannelId, "channel") },
      );

    if (!config?.defaultRoleId) {
      embed.setDescription("⚠️ Le rôle ciblé est obligatoire pour créer des présences.");
    }

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  // La lecture est ouverte à tous, la modification réservée aux gestionnaires.
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await upsertPresenceConfig({
    guildId: interaction.guildId,
    defaultRoleId: role?.id,
    exceptionRoleId: exceptionRole?.id,
    presenceChannelId: presenceChannel?.id,
    reminderChannelId: reminderChannel?.id,
  });

  await interaction.reply({
    content: "Configuration mise à jour. `/presence config` sans option pour la consulter.",
    flags: MessageFlags.Ephemeral,
  });
}
