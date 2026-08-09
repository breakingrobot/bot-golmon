import { prisma } from "../../lib/prisma.js";

/**
 * Membres à ne jamais ping pour les présences, marqués absents d'office.
 * Module séparé du service et des absences : il ne dépend que de Prisma, ce qui
 * évite un cycle d'imports entre les deux.
 */
export async function addToBlocklist(params: {
  guildId: string;
  userId: string;
  raison?: string;
  addedById: string;
}) {
  return prisma.presenceBlocklist.upsert({
    where: { guildId_userId: { guildId: params.guildId, userId: params.userId } },
    create: params,
    update: { raison: params.raison, addedById: params.addedById },
  });
}

export async function removeFromBlocklist(guildId: string, userId: string) {
  const { count } = await prisma.presenceBlocklist.deleteMany({ where: { guildId, userId } });
  return count > 0;
}

export async function listBlocklist(guildId: string) {
  return prisma.presenceBlocklist.findMany({ where: { guildId }, orderBy: { createdAt: "asc" } });
}

export async function getBlocklistedUserIds(guildId: string): Promise<string[]> {
  const rows = await prisma.presenceBlocklist.findMany({
    where: { guildId },
    select: { userId: true },
  });
  return rows.map((row) => row.userId);
}
