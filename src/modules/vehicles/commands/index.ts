import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import { buildVehicleLookupEmbed } from "../../../lib/embeds/vehicleLookupEmbed.js";
import { lookupPlate, normalizePlate } from "../api.js";
import {
  addVehicle,
  getVehicleByPlate,
  getVehicleConfig,
  removeVehicle,
  searchVehicles,
  setVehicleOwner,
  syncVehicleDashboard,
  upsertVehicleConfig,
} from "../service.js";

const GUILD_ONLY = {
  content: "Commande utilisable en serveur uniquement.",
  flags: MessageFlags.Ephemeral,
} as const;

export async function autocompleteVehicle(interaction: AutocompleteInteraction) {
  if (!interaction.guildId) {
    await interaction.respond([]);
    return;
  }
  const vehicles = await searchVehicles(interaction.guildId, interaction.options.getFocused());
  await interaction.respond(
    vehicles.map((vehicle) => ({
      name: `${vehicle.plate}${vehicle.modele ? ` — ${vehicle.modele}` : ""}`.slice(0, 100),
      value: vehicle.plate,
    })),
  );
}

export async function handleVehicleAdd(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const plate = normalizePlate(interaction.options.getString("plaque", true));
  if (!plate) {
    await interaction.reply({ content: "Plaque invalide.", flags: MessageFlags.Ephemeral });
    return;
  }

  const owner = interaction.options.getMentionable("proprietaire", true) as any;
  const ownerId = owner.id;
  const ownerIsRole = "color" in owner || "hoist" in owner;
  let modele = interaction.options.getString("modele") ?? undefined;

  const existing = await getVehicleByPlate(interaction.guildId, plate);
  if (existing) {
    await interaction.reply({
      content: `La plaque \`${plate}\` est déjà enregistrée. Utilise \`/vehicle owner\` pour la transférer.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  // Pré-remplissage : une API indisponible ne doit jamais bloquer l'ajout.
  let apiNote = "";
  if (!modele) {
    const result = await lookupPlate(plate);
    if (result.status === "found") {
      modele = result.vehicle.name ?? result.vehicle.model ?? undefined;
    } else if (result.status === "not_found") {
      apiNote = " (plaque inconnue de l'API, modèle non renseigné)";
    } else {
      apiNote = " (API injoignable, modèle non renseigné)";
    }
  }

  await addVehicle({ guildId: interaction.guildId, plate, ownerId, ownerIsRole, modele });
  await syncVehicleDashboard(interaction.client, interaction.guildId);

  const ownerMention = ownerIsRole ? `<@&${ownerId}>` : `<@${ownerId}>`;
  await interaction.editReply(
    `\`${plate}\`${modele ? ` — ${modele}` : ""} enregistré pour ${ownerMention}${apiNote}.`,
  );
}

export async function handleVehicleRemove(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const plate = normalizePlate(interaction.options.getString("plaque", true));
  const existing = await getVehicleByPlate(interaction.guildId, plate);
  if (!existing) {
    await interaction.reply({
      content: `Aucun véhicule enregistré avec la plaque \`${plate}\`.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await removeVehicle(interaction.guildId, plate);
  await syncVehicleDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`\`${plate}\` retiré du registre.`);
}

export async function handleVehicleOwner(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const plate = normalizePlate(interaction.options.getString("plaque", true));
  const owner = interaction.options.getMentionable("proprietaire", true) as any;
  const ownerId = owner.id;
  const ownerIsRole = "color" in owner || "hoist" in owner;

  const existing = await getVehicleByPlate(interaction.guildId, plate);
  if (!existing) {
    await interaction.reply({
      content: `Aucun véhicule enregistré avec la plaque \`${plate}\`.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await setVehicleOwner(interaction.guildId, plate, ownerId, ownerIsRole);
  await syncVehicleDashboard(interaction.client, interaction.guildId);
  const ownerMention = ownerIsRole ? `<@&${ownerId}>` : `<@${ownerId}>`;
  await interaction.editReply(`\`${plate}\` appartient désormais à ${ownerMention}.`);
}

export async function handleVehicleLookup(interaction: ChatInputCommandInteraction) {
  const plate = normalizePlate(interaction.options.getString("plaque", true));
  if (!plate) {
    await interaction.reply({ content: "Plaque invalide.", flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const result = await lookupPlate(plate);
  if (result.status === "not_found") {
    await interaction.editReply(`Aucun véhicule trouvé pour la plaque \`${plate}\`.`);
    return;
  }
  if (result.status === "error") {
    await interaction.editReply(
      `Impossible d'interroger l'API pour le moment (${result.reason}). Réessaie plus tard.`,
    );
    return;
  }

  const registered = interaction.guildId
    ? await getVehicleByPlate(interaction.guildId, plate)
    : null;

  await interaction.editReply({ embeds: [buildVehicleLookupEmbed(result.vehicle, registered)] });
}

export async function handleVehicleConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const dashboardChannel = interaction.options.getChannel("dashboard_channel");

  if (!dashboardChannel) {
    const config = await getVehicleConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration des véhicules")
      .setColor(0x5865f2)
      .addFields({
        name: "Salon du tableau de bord",
        value: config?.dashboardChannelId ? `<#${config.dashboardChannelId}>` : "*non configuré*",
      });
    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await upsertVehicleConfig({
    guildId: interaction.guildId,
    dashboardChannelId: dashboardChannel.id,
    dashboardMessageId: null,
  });
  await syncVehicleDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Tableau de bord installé dans <#${dashboardChannel.id}>.`);
}
