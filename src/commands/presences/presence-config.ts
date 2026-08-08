import { ChannelType, PermissionFlagsBits, SlashCommandBuilder } from "discord.js";
import type { Command } from "../../types/command.js";
import { upsertPresenceConfig } from "../../modules/presences/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence-config")
    .setDescription("Configure le module présences pour ce serveur")
    .setDefaultMemberPermissions(PermissionFlagsBits.ManageGuild)
    .addRoleOption((option) =>
      option.setName("role").setDescription("Rôle ciblé par défaut").setRequired(true),
    )
    .addRoleOption((option) =>
      option.setName("exception_role").setDescription("Rôle d'exception (auto-absent, non ping)"),
    )
    .addChannelOption((option) =>
      option
        .setName("reminder_channel")
        .setDescription("Salon des rappels automatiques")
        .addChannelTypes(ChannelType.GuildText),
    ),
  async execute(interaction) {
    if (!interaction.guildId) {
      await interaction.reply({ content: "Commande utilisable en serveur uniquement.", ephemeral: true });
      return;
    }

    const role = interaction.options.getRole("role", true);
    const exceptionRole = interaction.options.getRole("exception_role");
    const reminderChannel = interaction.options.getChannel("reminder_channel");

    await upsertPresenceConfig({
      guildId: interaction.guildId,
      defaultRoleId: role.id,
      exceptionRoleId: exceptionRole?.id,
      reminderChannelId: reminderChannel?.id,
    });

    await interaction.reply({ content: "Configuration mise à jour.", ephemeral: true });
  },
};
