import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import { Prisma } from "@prisma/client";
import { buildVehicleLookupEmbed } from "../../../lib/embeds/vehicleLookupEmbed.js";
import { ownerMention } from "../../../lib/format.js";
import { GUILD_ONLY } from "../../../lib/replies.js";
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

/**
 * L'option est de type `Mentionable` : discord.js renseigne `role` ou `user`
 * selon ce qui a été mentionné. On lit ces champs plutôt que de deviner le type
 * de l'objet renvoyé par `getMentionable`, dont l'union n'est pas discriminée.
 */
function resolveOwner(interaction: ChatInputCommandInteraction): {
  ownerId: string;
  ownerIsRole: boolean;
} {
  const option = interaction.options.get("proprietaire", true);
  if (option.role) {
    return { ownerId: option.role.id, ownerIsRole: true };
  }
  if (option.user) {
    return { ownerId: option.user.id, ownerIsRole: false };
  }
  throw new Error("L'option « proprietaire » n'est ni un membre ni un rôle");
}

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

  const { ownerId, ownerIsRole } = resolveOwner(interaction);
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

  try {
    await addVehicle({ guildId: interaction.guildId, plate, ownerId, ownerIsRole, modele });
  } catch (error) {
    // La consultation de l'API laisse une fenêtre pendant laquelle la plaque a
    // pu être enregistrée par quelqu'un d'autre.
    if (
      error instanceof Prisma.PrismaClientKnownRequestError &&
      error.code === "P2002"
    ) {
      await interaction.editReply(
        `La plaque \`${plate}\` est déjà enregistrée. Utilise \`/vehicle owner\` pour la transférer.`,
      );
      return;
    }
    throw error;
  }

  await syncVehicleDashboard(interaction.client, interaction.guildId);

  await interaction.editReply(
    `\`${plate}\`${modele ? ` — ${modele}` : ""} enregistré pour ${ownerMention(ownerId, ownerIsRole)}${apiNote}.`,
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
  const { ownerId, ownerIsRole } = resolveOwner(interaction);

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
  await interaction.editReply(
    `\`${plate}\` appartient désormais à ${ownerMention(ownerId, ownerIsRole)}.`,
  );
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

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await upsertVehicleConfig({
    guildId: interaction.guildId,
    dashboardChannelId: dashboardChannel.id,
    dashboardMessageId: null,
  });
  await syncVehicleDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Tableau de bord installé dans <#${dashboardChannel.id}>.`);
}
