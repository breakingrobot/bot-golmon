import type { Client } from "discord.js";
import type { Vehicle } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { fetchChannelSafe } from "../../lib/discordClient.js";
import { buildVehicleDashboardEmbed } from "../../lib/embeds/vehicleDashboardEmbed.js";
import { normalizePlate } from "./api.js";

export async function getVehicleConfig(guildId: string) {
  return prisma.vehicleConfig.findUnique({ where: { guildId } });
}

export async function upsertVehicleConfig(params: {
  guildId: string;
  dashboardChannelId?: string;
  dashboardMessageId?: string | null;
}) {
  const { guildId, ...data } = params;
  return prisma.vehicleConfig.upsert({
    where: { guildId },
    create: { guildId, ...data },
    update: data,
  });
}

export async function getVehicleByPlate(guildId: string, plate: string) {
  return prisma.vehicle.findUnique({
    where: { guildId_plate: { guildId, plate: normalizePlate(plate) } },
  });
}

export async function addVehicle(params: {
  guildId: string;
  plate: string;
  ownerId: string;
  ownerIsRole: boolean;
  modele?: string;
}) {
  return prisma.vehicle.create({
    data: { ...params, plate: normalizePlate(params.plate) },
  });
}

export async function setVehicleOwner(guildId: string, plate: string, ownerId: string, ownerIsRole: boolean) {
  return prisma.vehicle.update({
    where: { guildId_plate: { guildId, plate: normalizePlate(plate) } },
    data: { ownerId, ownerIsRole },
  });
}

export async function removeVehicle(guildId: string, plate: string) {
  return prisma.vehicle.delete({
    where: { guildId_plate: { guildId, plate: normalizePlate(plate) } },
  });
}

export async function listVehicles(guildId: string): Promise<Vehicle[]> {
  return prisma.vehicle.findMany({
    where: { guildId },
    orderBy: [{ ownerId: "asc" }, { plate: "asc" }],
  });
}

export async function searchVehicles(guildId: string, query: string) {
  return prisma.vehicle.findMany({
    where: { guildId, plate: { contains: normalizePlate(query) } },
    take: 25,
    orderBy: { plate: "asc" },
  });
}

/**
 * Met à jour (ou crée) le message unique listant tous les véhicules de la
 * guilde. Sans salon configuré, ne fait rien. Même mécanique que le tableau de
 * bord des transactions.
 */
export async function syncVehicleDashboard(client: Client, guildId: string) {
  const config = await getVehicleConfig(guildId);
  if (!config?.dashboardChannelId) {
    return;
  }

  const channel = await fetchChannelSafe(
    client,
    config.dashboardChannelId,
    `le salon de tableau de bord des véhicules de la guilde ${guildId}`,
  );
  if (!channel?.isSendable()) {
    logger.warn(`Salon du tableau de bord véhicules introuvable pour la guilde ${guildId}`);
    return;
  }

  const embed = buildVehicleDashboardEmbed(await listVehicles(guildId));

  if (config.dashboardMessageId) {
    const message = await channel.messages.fetch(config.dashboardMessageId).catch(() => null);
    if (message) {
      await message.edit({ embeds: [embed] });
      return;
    }
    // Message supprimé : on en repost un et on mémorise le nouvel identifiant.
  }

  const message = await channel.send({ embeds: [embed] });
  await upsertVehicleConfig({ guildId, dashboardMessageId: message.id });
}
