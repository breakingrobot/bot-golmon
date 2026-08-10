import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type ChatInputCommandInteraction,
} from "discord.js";
import { GUILD_ONLY } from "../../../lib/replies.js";
import {
  getPhoneConfig,
  getPhoneNumber,
  NUMBER_TAKEN,
  removePhoneNumber,
  setPhoneNumber,
  syncPhoneDashboard,
  upsertPhoneConfig,
} from "../service.js";
import { formatPhone, INVALID_FORMAT, normalizePhone, PHONE_REGEX } from "../constants.js";

const MANAGE_GUILD_ONLY = {
  content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
  flags: MessageFlags.Ephemeral,
} as const;

export async function handlePhoneConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const dashboardChannel = interaction.options.getChannel("salon");

  // Sans option, la commande sert à consulter la configuration : lecture ouverte
  // à tous, modification réservée aux gestionnaires (comme les autres modules).
  if (!dashboardChannel) {
    const config = await getPhoneConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration de l'annuaire")
      .setColor(0x5865f2)
      .addFields({
        name: "Salon de l'annuaire",
        value: config?.dashboardChannelId ? `<#${config.dashboardChannelId}>` : "*non configuré*",
      });
    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply(MANAGE_GUILD_ONLY);
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await upsertPhoneConfig({
    guildId: interaction.guildId,
    dashboardChannelId: dashboardChannel.id,
    dashboardMessageId: null,
  });
  await syncPhoneDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Annuaire installé dans <#${dashboardChannel.id}>.`);
}

export async function handlePhoneAjouter(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  const numero = normalizePhone(interaction.options.getString("numero", true));
  if (!PHONE_REGEX.test(numero)) {
    await interaction.reply({ content: INVALID_FORMAT, flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const result = await setPhoneNumber(interaction.guildId, interaction.user.id, numero);
  if (result.status === "taken") {
    await interaction.editReply(NUMBER_TAKEN);
    return;
  }

  await syncPhoneDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Numéro enregistré : \`${formatPhone(numero)}\``);
}

export async function handlePhoneVoir(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  const membre = interaction.options.getUser("membre") ?? interaction.user;
  const found = await getPhoneNumber(interaction.guildId, membre.id);

  await interaction.reply({
    content: found
      ? `Numéro de ${membre.username} : \`${formatPhone(found.numero)}\``
      : `${membre.username} n'a pas de numéro enregistré.`,
    flags: MessageFlags.Ephemeral,
  });
}

export async function handlePhoneSupprimer(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  const membre = interaction.options.getUser("membre");
  const targetId = membre ? membre.id : interaction.user.id;
  
  if (membre && membre.id !== interaction.user.id && !interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Tu n'as pas la permission de supprimer le numéro d'un autre membre.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }
  
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  const removed = await removePhoneNumber(interaction.guildId, targetId);
  if (removed) {
    await syncPhoneDashboard(interaction.client, interaction.guildId);
    await interaction.editReply("Numéro supprimé avec succès.");
  } else {
    await interaction.editReply("Aucun numéro enregistré pour ce membre.");
  }
}

export async function handlePhoneListe(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await syncPhoneDashboard(interaction.client, interaction.guildId);
  await interaction.editReply("Annuaire republié dans le salon configuré.");
}
