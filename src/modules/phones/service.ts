import type { Client } from "discord.js";
import { Prisma } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { fetchChannelSafe } from "../../lib/discordClient.js";
import { buildPhoneDashboardEmbed } from "../../lib/embeds/phoneDashboardEmbed.js";
import { buildPhoneButtons } from "../../lib/components/phoneButtons.js";

export async function getPhoneConfig(guildId: string) {
  return prisma.phoneConfig.findUnique({ where: { guildId } });
}

export async function upsertPhoneConfig(params: {
  guildId: string;
  dashboardChannelId?: string;
  dashboardMessageId?: string | null;
}) {
  const { guildId, ...data } = params;
  return prisma.phoneConfig.upsert({
    where: { guildId },
    create: { guildId, ...data },
    update: data,
  });
}

/**
 * Un numéro est unique dans la guilde : l'échec d'unicité est un cas métier
 * normal, renvoyé plutôt que jeté, pour que l'appelant formule le message.
 */
export type SetPhoneResult = { status: "ok" } | { status: "taken" };

export async function setPhoneNumber(
  guildId: string,
  userId: string,
  numero: string,
): Promise<SetPhoneResult> {
  try {
    await prisma.phoneNumber.upsert({
      where: { guildId_userId: { guildId, userId } },
      update: { numero },
      create: { guildId, userId, numero },
    });
    return { status: "ok" };
  } catch (error) {
    if (error instanceof Prisma.PrismaClientKnownRequestError && error.code === "P2002") {
      return { status: "taken" };
    }
    throw error;
  }
}

export const NUMBER_TAKEN = "Ce numéro est déjà attribué à un autre membre.";

export async function removePhoneNumber(guildId: string, userId: string) {
  const result = await prisma.phoneNumber.deleteMany({
    where: { guildId, userId },
  });
  return result.count > 0;
}

export async function getPhoneNumber(guildId: string, userId: string) {
  return prisma.phoneNumber.findUnique({ where: { guildId_userId: { guildId, userId } } });
}

export async function listPhoneNumbers(guildId: string) {
  return prisma.phoneNumber.findMany({
    where: { guildId },
    orderBy: { numero: "asc" },
  });
}

export async function syncPhoneDashboard(client: Client, guildId: string) {
  const config = await getPhoneConfig(guildId);
  if (!config?.dashboardChannelId) return;

  const channel = await fetchChannelSafe(
    client,
    config.dashboardChannelId,
    "le salon de l'annuaire téléphonique",
  );
  if (!channel?.isSendable()) return;

  const numbers = await listPhoneNumbers(guildId);
  const embeds = buildPhoneDashboardEmbed(numbers);
  const components = buildPhoneButtons(guildId);

  try {
    if (config.dashboardMessageId) {
      const message = await channel.messages.fetch(config.dashboardMessageId).catch(() => null);
      if (message && message.editable) {
        await message.edit({ embeds, components: [components] });
        return;
      }
    }
    const newMessage = await channel.send({ embeds, components: [components] });
    await prisma.phoneConfig.update({
      where: { guildId },
      data: { dashboardMessageId: newMessage.id },
    });
  } catch (error) {
    logger.error(`Erreur syncPhoneDashboard pour la guilde ${guildId}:`, error);
  }
}
