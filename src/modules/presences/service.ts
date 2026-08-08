import type { Client, Guild, TextChannel } from "discord.js";
import { ResponseStatus } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { buildPresenceEmbed } from "../../lib/embeds/presenceEmbed.js";
import { buildPresenceButtons } from "../../lib/components/presenceButtons.js";

export async function getPresenceConfig(guildId: string) {
  return prisma.presenceConfig.findUnique({ where: { guildId } });
}

export async function upsertPresenceConfig(params: {
  guildId: string;
  defaultRoleId?: string;
  exceptionRoleId?: string;
  reminderChannelId?: string;
}) {
  const { guildId, ...data } = params;
  return prisma.presenceConfig.upsert({
    where: { guildId },
    create: { guildId, ...data },
    update: data,
  });
}

async function getRoleMemberIds(guild: Guild, roleId: string): Promise<string[]> {
  await guild.members.fetch();
  const role = guild.roles.cache.get(roleId);
  if (!role) {
    return [];
  }
  return [...role.members.keys()];
}

export async function createPresence(
  guild: Guild,
  params: {
    channelId: string;
    titre: string;
    description?: string;
    eventDate: Date;
    roleId: string;
    exceptionRoleId?: string;
    createdById: string;
  },
) {
  const presence = await prisma.presence.create({
    data: {
      guildId: guild.id,
      channelId: params.channelId,
      titre: params.titre,
      description: params.description,
      eventDate: params.eventDate,
      roleId: params.roleId,
      exceptionRoleId: params.exceptionRoleId,
      createdById: params.createdById,
    },
  });

  if (params.exceptionRoleId) {
    const exceptionMemberIds = await getRoleMemberIds(guild, params.exceptionRoleId);
    for (const userId of exceptionMemberIds) {
      await registerResponse(presence.id, userId, ResponseStatus.ABSENT);
    }
  }

  return presence;
}

export async function registerResponse(
  presenceId: string,
  userId: string,
  status: ResponseStatus,
  retardMinutes?: number,
) {
  const existing = await prisma.presenceResponse.findUnique({
    where: { presenceId_userId: { presenceId, userId } },
  });

  const response = await prisma.presenceResponse.upsert({
    where: { presenceId_userId: { presenceId, userId } },
    create: { presenceId, userId, status, retardMinutes },
    update: { status, retardMinutes, respondedAt: new Date() },
  });

  await prisma.presenceResponseChange.create({
    data: {
      responseId: response.id,
      fromStatus: existing?.status,
      toStatus: status,
    },
  });

  return response;
}

export async function getPresenceWithResponses(presenceId: string) {
  return prisma.presence.findUnique({
    where: { id: presenceId },
    include: { responses: true },
  });
}

export async function computeEffectifTotal(guild: Guild, roleId: string): Promise<number> {
  const memberIds = await getRoleMemberIds(guild, roleId);
  return memberIds.length;
}

export async function getNonRespondentIds(guild: Guild, presence: { id: string; roleId: string }) {
  const memberIds = await getRoleMemberIds(guild, presence.roleId);
  const responses = await prisma.presenceResponse.findMany({
    where: { presenceId: presence.id },
    select: { userId: true },
  });
  const responded = new Set(responses.map((r) => r.userId));
  return memberIds.filter((id) => !responded.has(id));
}

export async function closePresence(presenceId: string) {
  return prisma.presence.update({ where: { id: presenceId }, data: { closedAt: new Date() } });
}

export async function findOpenPresences(guildId: string, query: string) {
  return prisma.presence.findMany({
    where: { guildId, closedAt: null, titre: { contains: query } },
    take: 25,
    orderBy: { eventDate: "asc" },
  });
}

export async function computeUserStats(guildId: string, userId: string) {
  const closedPresences = await prisma.presence.findMany({
    where: { guildId, closedAt: { not: null } },
    select: { id: true },
  });
  const totalClosed = closedPresences.length;

  const responses = await prisma.presenceResponse.findMany({
    where: { userId, presence: { guildId, closedAt: { not: null } } },
  });

  const present = responses.filter((r) => r.status === ResponseStatus.PRESENT).length;
  const absent = responses.filter((r) => r.status === ResponseStatus.ABSENT).length;
  const retard = responses.filter((r) => r.status === ResponseStatus.RETARD).length;
  const sansReponse = totalClosed - responses.length;
  const percentPresence = totalClosed > 0 ? Math.round((present / totalClosed) * 100) : 0;

  const recentChanges = await prisma.presenceResponseChange.findMany({
    where: { response: { userId, presence: { guildId } } },
    include: { response: { include: { presence: true } } },
    orderBy: { changedAt: "desc" },
    take: 5,
  });

  return { totalClosed, present, absent, retard, sansReponse, percentPresence, recentChanges };
}

export async function computeGuildLeaderboard(guildId: string) {
  const grouped = await prisma.presenceResponse.groupBy({
    by: ["userId", "status"],
    where: { presence: { guildId, closedAt: { not: null } } },
    _count: true,
  });

  const totalClosed = await prisma.presence.count({ where: { guildId, closedAt: { not: null } } });

  const byUser = new Map<string, { present: number; absent: number; retard: number }>();
  for (const row of grouped) {
    const entry = byUser.get(row.userId) ?? { present: 0, absent: 0, retard: 0 };
    if (row.status === ResponseStatus.PRESENT) entry.present = row._count;
    if (row.status === ResponseStatus.ABSENT) entry.absent = row._count;
    if (row.status === ResponseStatus.RETARD) entry.retard = row._count;
    byUser.set(row.userId, entry);
  }

  return [...byUser.entries()]
    .map(([userId, counts]) => ({
      userId,
      ...counts,
      percentPresence: totalClosed > 0 ? Math.round((counts.present / totalClosed) * 100) : 0,
    }))
    .sort((a, b) => b.percentPresence - a.percentPresence);
}

export async function syncPresenceMessage(client: Client, presenceId: string) {
  const presence = await getPresenceWithResponses(presenceId);
  if (!presence) {
    return;
  }

  const guild = await client.guilds.fetch(presence.guildId);
  const channel = await client.channels.fetch(presence.channelId);
  if (!channel?.isTextBased()) {
    return;
  }

  const effectifTotal = await computeEffectifTotal(guild, presence.roleId);
  const embed = buildPresenceEmbed(presence, guild.iconURL(), effectifTotal);
  const counts = {
    present: presence.responses.filter((r) => r.status === ResponseStatus.PRESENT).length,
    absent: presence.responses.filter((r) => r.status === ResponseStatus.ABSENT).length,
    retard: presence.responses.filter((r) => r.status === ResponseStatus.RETARD).length,
  };
  const components = [buildPresenceButtons(presence.id, counts, presence.closedAt !== null)];

  if (presence.messageId) {
    try {
      const message = await channel.messages.fetch(presence.messageId);
      await message.edit({ embeds: [embed], components });
      return;
    } catch {
      // message introuvable -> on en poste un nouveau ci-dessous
    }
  }

  const message = await (channel as TextChannel).send({ embeds: [embed], components });
  await prisma.presence.update({ where: { id: presence.id }, data: { messageId: message.id } });
}
