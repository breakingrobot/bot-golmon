import type { Client, Guild } from "discord.js";
import type { ProspectVoteChoice } from "@prisma/client";
import { startOfWeek } from "date-fns";
import { fromZonedTime, toZonedTime } from "date-fns-tz";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { fetchChannelSafe, fetchGuildSafe } from "../../lib/discordClient.js";
import { TIMEZONE } from "../../lib/datetime.js";
import { getRoleMemberIds } from "../../lib/memberCache.js";
import { buildProspectDashboardEmbed } from "../../lib/embeds/prospectDashboardEmbed.js";
import { buildProspectComponents } from "../../lib/components/prospectComponents.js";

export async function getProspectConfig(guildId: string) {
  return prisma.prospectConfig.findUnique({ where: { guildId } });
}

export async function upsertProspectConfig(params: {
  guildId: string;
  prospectRoleId?: string;
  gradeRoleId?: string;
  leadRoleId?: string;
  categoryId?: string;
  dashboardChannelId?: string;
}) {
  const { guildId, ...data } = params;
  return prisma.prospectConfig.upsert({
    where: { guildId },
    create: { guildId, ...data },
    update: data,
  });
}

/**
 * Lundi 00:00 heure de Paris, exprimé en UTC. Le conteneur tourne en UTC : sans
 * conversion, un dimanche 23:30 à Paris serait rattaché à la semaine suivante.
 */
export function currentWeekStart(reference: Date = new Date()): Date {
  const local = startOfWeek(toZonedTime(reference, TIMEZONE), { weekStartsOn: 1 });
  const year = local.getFullYear();
  const month = String(local.getMonth() + 1).padStart(2, "0");
  const day = String(local.getDate()).padStart(2, "0");
  return fromZonedTime(`${year}-${month}-${day}T00:00:00`, TIMEZONE);
}

export type SessionWithEntries = NonNullable<Awaited<ReturnType<typeof getSession>>>;

export async function getSession(sessionId: string) {
  return prisma.prospectSession.findUnique({
    where: { id: sessionId },
    include: {
      entries: {
        include: { votes: { orderBy: { createdAt: "asc" } } },
        orderBy: { createdAt: "asc" },
      },
    },
  });
}

export async function getCurrentSession(guildId: string) {
  const session = await prisma.prospectSession.findUnique({
    where: { guildId_weekStart: { guildId, weekStart: currentWeekStart() } },
    select: { id: true },
  });
  return session ? getSession(session.id) : null;
}

/**
 * Garantit qu'une session existe pour la semaine en cours, en clôturant les
 * précédentes. Idempotent : appelable par le cron, par une commande ou à
 * l'arrivée d'un prospect sans risque de doublon.
 *
 * La nouvelle session est amorcée avec les membres actuels du rôle prospect.
 */
export async function ensureCurrentSession(guild: Guild): Promise<SessionWithEntries> {
  const weekStart = currentWeekStart();

  const existing = await prisma.prospectSession.findUnique({
    where: { guildId_weekStart: { guildId: guild.id, weekStart } },
    select: { id: true },
  });
  if (existing) {
    return (await getSession(existing.id)) as SessionWithEntries;
  }

  // Les semaines écoulées sont conservées, seulement marquées closes.
  await prisma.prospectSession.updateMany({
    where: { guildId: guild.id, closedAt: null },
    data: { closedAt: new Date() },
  });

  const config = await getProspectConfig(guild.id);
  const prospectIds = config?.prospectRoleId
    ? await getRoleMemberIds(guild, config.prospectRoleId)
    : [];

  const session = await prisma.prospectSession.create({
    data: {
      guildId: guild.id,
      weekStart,
      entries: {
        create: prospectIds.map((userId) => ({ guildId: guild.id, userId })),
      },
    },
  });

  logger.info(
    `Nouvelle session de prospects pour ${guild.name} (${prospectIds.length} prospect(s))`,
  );

  return (await getSession(session.id)) as SessionWithEntries;
}

export async function addEntry(sessionId: string, guildId: string, userId: string) {
  return prisma.prospectEntry.upsert({
    where: { sessionId_userId: { sessionId, userId } },
    create: { sessionId, guildId, userId },
    update: {},
  });
}

export async function removeEntry(sessionId: string, userId: string) {
  return prisma.prospectEntry.deleteMany({ where: { sessionId, userId } });
}

/** Un avis par gradé et par prospect : revoter remplace l'avis précédent. */
export async function castVote(
  entryId: string,
  voterId: string,
  choix: ProspectVoteChoice,
  motif: string,
) {
  return prisma.prospectVote.upsert({
    where: { entryId_voterId: { entryId, voterId } },
    create: { entryId, voterId, choix, motif },
    update: { choix, motif },
  });
}

export async function setVeto(entryId: string, leadId: string, motif: string) {
  return prisma.prospectEntry.update({
    where: { id: entryId },
    data: { vetoById: leadId, vetoMotif: motif, vetoAt: new Date() },
  });
}

export async function clearVeto(entryId: string) {
  return prisma.prospectEntry.update({
    where: { id: entryId },
    data: { vetoById: null, vetoMotif: null, vetoAt: null },
  });
}

export async function getEntry(entryId: string) {
  return prisma.prospectEntry.findUnique({
    where: { id: entryId },
    include: { votes: { orderBy: { createdAt: "asc" } }, session: true },
  });
}

export async function findEntryInCurrentSession(guildId: string, userId: string) {
  const session = await prisma.prospectSession.findUnique({
    where: { guildId_weekStart: { guildId, weekStart: currentWeekStart() } },
    select: { id: true },
  });
  if (!session) {
    return null;
  }
  return prisma.prospectEntry.findUnique({
    where: { sessionId_userId: { sessionId: session.id, userId } },
    include: { votes: { orderBy: { createdAt: "asc" } }, session: true },
  });
}

/** Semaines passées d'un membre, de la plus récente à la plus ancienne. */
export async function getUserHistory(guildId: string, userId: string, take = 8) {
  return prisma.prospectEntry.findMany({
    where: { guildId, userId },
    include: { votes: true, session: true },
    orderBy: { createdAt: "desc" },
    take,
  });
}

export async function getProspectChannelId(guildId: string, userId: string) {
  const record = await prisma.prospectChannel.findUnique({
    where: { guildId_userId: { guildId, userId } },
  });
  return record?.channelId ?? null;
}

export async function rememberProspectChannel(
  guildId: string,
  userId: string,
  channelId: string,
) {
  return prisma.prospectChannel.upsert({
    where: { guildId_userId: { guildId, userId } },
    create: { guildId, userId, channelId },
    update: { channelId },
  });
}

export async function forgetProspectChannel(guildId: string, userId: string) {
  return prisma.prospectChannel.deleteMany({ where: { guildId, userId } });
}

/**
 * Pseudos des prospects pour les libellés des select menus, lus dans le cache
 * des membres (tenu à jour par l'intent GuildMembers). Un membre parti n'y est
 * plus : l'appelant retombe alors sur son identifiant.
 */
export function resolveLabels(
  guild: Guild | null,
  session: SessionWithEntries,
): Map<string, string> {
  return new Map(
    session.entries.flatMap((entry) => {
      const member = guild?.members.cache.get(entry.userId);
      return member ? [[entry.userId, member.displayName] as const] : [];
    }),
  );
}

/**
 * Met à jour le message d'avis de la semaine en cours. Contrairement aux autres
 * tableaux de bord, l'identifiant du message est porté par la *session* et non
 * par la config : chaque semaine a donc son propre message, et ceux des
 * semaines écoulées restent visibles dans le salon comme archive.
 */
export async function syncProspectDashboard(client: Client, guildId: string) {
  const config = await getProspectConfig(guildId);
  if (!config?.dashboardChannelId) {
    return;
  }

  const session = await getCurrentSession(guildId);
  if (!session) {
    return;
  }

  const channel = await fetchChannelSafe(
    client,
    config.dashboardChannelId,
    "le salon des avis prospects",
  );
  if (!channel?.isSendable()) {
    logger.warn(`Salon des avis prospects introuvable pour la guilde ${guildId}`);
    return;
  }

  const guild = await fetchGuildSafe(client, guildId, `la guilde ${guildId}`);
  const payload = {
    embeds: [buildProspectDashboardEmbed(session)],
    components: buildProspectComponents(session, resolveLabels(guild, session)),
  };

  // Le message a pu être supprimé : on repart d'un message neuf.
  if (session.messageId) {
    const message = await channel.messages.fetch(session.messageId).catch(() => null);
    if (message && message.editable) {
      await message.edit(payload);
      return;
    }
    // Message supprimé : on en republie un et on mémorise le nouvel identifiant.
  }

  const message = await channel.send(payload);
  await prisma.prospectSession.update({
    where: { id: session.id },
    data: { messageId: message.id },
  });
}
