import { ChannelType, SlashCommandBuilder, type TextChannel } from "discord.js";
import type { Command } from "../../types/command.js";
import { createPresence, getPresenceConfig, syncPresenceMessage } from "../../modules/presences/service.js";

function parseDateHeure(date: string, heure: string): Date | null {
  const dateMatch = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(date);
  const heureMatch = /^(\d{2}):(\d{2})$/.exec(heure);
  if (!dateMatch || !heureMatch) {
    return null;
  }
  const [, day, month, year] = dateMatch;
  const [, hours, minutes] = heureMatch;
  const parsed = new Date(
    Number(year),
    Number(month) - 1,
    Number(day),
    Number(hours),
    Number(minutes),
  );
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("presence-create")
    .setDescription("Crée une nouvelle présence")
    .addStringOption((option) => option.setName("titre").setDescription("Titre").setRequired(true))
    .addStringOption((option) =>
      option.setName("date").setDescription("Date au format JJ/MM/AAAA").setRequired(true),
    )
    .addStringOption((option) =>
      option.setName("heure").setDescription("Heure au format HH:MM").setRequired(true),
    )
    .addStringOption((option) => option.setName("description").setDescription("Description"))
    .addRoleOption((option) => option.setName("role").setDescription("Rôle ciblé (override)"))
    .addRoleOption((option) =>
      option.setName("exception_role").setDescription("Rôle d'exception (override)"),
    )
    .addChannelOption((option) =>
      option
        .setName("channel")
        .setDescription("Salon d'affichage (par défaut: salon actuel)")
        .addChannelTypes(ChannelType.GuildText),
    ),
  async execute(interaction) {
    if (!interaction.guild) {
      await interaction.reply({ content: "Commande utilisable en serveur uniquement.", ephemeral: true });
      return;
    }

    const titre = interaction.options.getString("titre", true);
    const date = interaction.options.getString("date", true);
    const heure = interaction.options.getString("heure", true);
    const description = interaction.options.getString("description") ?? undefined;
    const roleOption = interaction.options.getRole("role");
    const exceptionRoleOption = interaction.options.getRole("exception_role");
    const channel = interaction.options.getChannel("channel") ?? interaction.channel;

    const eventDate = parseDateHeure(date, heure);
    if (!eventDate) {
      await interaction.reply({
        content: "Format de date/heure invalide. Attendu: JJ/MM/AAAA et HH:MM.",
        ephemeral: true,
      });
      return;
    }

    const config = await getPresenceConfig(interaction.guild.id);
    const roleId = roleOption?.id ?? config?.defaultRoleId;
    const exceptionRoleId = exceptionRoleOption?.id ?? config?.exceptionRoleId ?? undefined;

    if (!roleId) {
      await interaction.reply({
        content: "Aucun rôle configuré. Utilisez /presence-config ou précisez un rôle.",
        ephemeral: true,
      });
      return;
    }

    if (!channel) {
      await interaction.reply({ content: "Salon introuvable.", ephemeral: true });
      return;
    }

    await interaction.deferReply({ ephemeral: true });

    const presence = await createPresence(interaction.guild, {
      channelId: channel.id,
      titre,
      description,
      eventDate,
      roleId,
      exceptionRoleId,
      createdById: interaction.user.id,
    });

    await syncPresenceMessage(interaction.client, presence.id);

    const role = interaction.guild.roles.cache.get(roleId);
    const excludedIds = new Set(
      exceptionRoleId ? interaction.guild.roles.cache.get(exceptionRoleId)?.members.map((m) => m.id) : [],
    );
    const mentionable = role
      ? [...role.members.values()].filter((m) => !excludedIds.has(m.id)).map((m) => `<@${m.id}>`)
      : [];

    if (mentionable.length > 0) {
      await (channel as TextChannel).send({
        content: mentionable.join(" "),
        allowedMentions: { parse: ["users"] },
      });
    }

    await interaction.editReply({ content: `Présence créée : \`${presence.id}\`` });
  },
};
