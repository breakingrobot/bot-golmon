import type { Client, Guild } from "discord.js";
import { ResponseStatus } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { getPresenceConfig, registerResponse, syncPresenceMessage } from "./service.js";
import { ensureMembersCached } from "../../lib/memberCache.js";

export interface RoleUpdateResult {
  ok: boolean;
  reason?: string;
}

/**
 * Applique ou retire le rôle d'absence. Le bot peut légitimement échouer
 * (permission ManageRoles absente, ou rôle plus haut que le sien dans la
 * hiérarchie) : on renvoie l'erreur au lieu de la laisser remonter, pour que
 * l'absence soit tout de même enregistrée et le problème signalé.
 */
async function updateAbsenceRole(
  guild: Guild,
  userId: string,
  action: "add" | "remove",
): Promise<RoleUpdateResult> {
  const config = await getPresenceConfig(guild.id);
  if (!config?.absenceRoleId) {
    return { ok: false, reason: "aucun rôle d'absence configuré" };
  }

  try {
    await ensureMembersCached(guild);
    const member = guild.members.cache.get(userId) ?? (await guild.members.fetch(userId));

    if (action === "add" && !member.roles.cache.has(config.absenceRoleId)) {
      await member.roles.add(config.absenceRoleId);
    } else if (action === "remove" && member.roles.cache.has(config.absenceRoleId)) {
      await member.roles.remove(config.absenceRoleId);
    }
    return { ok: true };
  } catch (error) {
    const reason = error instanceof Error ? error.message : "erreur inconnue";
    logger.warn(`Rôle d'absence (${action}) impossible pour ${userId}: ${reason}`);
    return { ok: false, reason };
  }
}

export async function getActiveAbsence(guildId: string, userId: string) {
  const now = new Date();
  return prisma.absence.findFirst({
    where: { guildId, userId, endedAt: null, startsAt: { lte: now }, endsAt: { gt: now } },
  });
}

export async function listActiveAbsences(guildId: string) {
  return prisma.absence.findMany({
    where: { guildId, endedAt: null, endsAt: { gt: new Date() } },
    orderBy: { endsAt: "asc" },
  });
}

/**
 * Enregistre une absence, attribue le rôle si elle a déjà commencé et marque
 * l'utilisateur absent sur les présences ouvertes tombant dans la période.
 */
export async function declareAbsence(
  client: Client,
  guild: Guild,
  params: {
    userId: string;
    motif: string;
    startsAt: Date;
    endsAt: Date;
    createdById: string;
  },
) {
  const absence = await prisma.absence.create({ data: { guildId: guild.id, ...params } });

  const roleResult =
    params.startsAt.getTime() <= Date.now()
      ? await updateAbsenceRole(guild, params.userId, "add")
      : { ok: true };

  const affected = await markAbsentOnOpenPresences(client, guild.id, params);

  return { absence, roleResult, affected };
}

/** Marque l'utilisateur absent sur les présences ouvertes couvertes par l'absence. */
async function markAbsentOnOpenPresences(
  client: Client,
  guildId: string,
  params: { userId: string; startsAt: Date; endsAt: Date },
): Promise<number> {
  const presences = await prisma.presence.findMany({
    where: {
      guildId,
      closedAt: null,
      eventDate: { gte: params.startsAt, lte: params.endsAt },
    },
    select: { id: true },
  });

  let affected = 0;
  for (const presence of presences) {
    const { changed } = await registerResponse(presence.id, params.userId, ResponseStatus.ABSENT);
    if (changed) {
      affected += 1;
      await syncPresenceMessage(client, presence.id).catch((error: unknown) => {
        logger.error(`Sync de la présence ${presence.id} après absence échouée`, error);
      });
    }
  }

  return affected;
}

/**
 * Clôture une absence. Le rôle n'est retiré que si l'utilisateur n'a aucune
 * autre absence encore active (périodes qui se chevauchent).
 */
export async function endAbsence(guild: Guild, absenceId: string, userId: string) {
  const stillAbsent = await prisma.absence.findFirst({
    where: {
      guildId: guild.id,
      userId,
      endedAt: null,
      id: { not: absenceId },
      startsAt: { lte: new Date() },
      endsAt: { gt: new Date() },
    },
  });

  if (stillAbsent) {
    await prisma.absence.update({ where: { id: absenceId }, data: { endedAt: new Date() } });
    return { ok: true, roleRemoved: false };
  }

  const roleResult = await updateAbsenceRole(guild, userId, "remove");

  if (roleResult.ok) {
    await prisma.absence.update({ where: { id: absenceId }, data: { endedAt: new Date() } });
  }

  return { ok: roleResult.ok, roleRemoved: roleResult.ok, reason: roleResult.reason };
}

/** Absences arrivées à échéance, à clôturer par le cron. */
export async function findExpiredAbsences() {
  return prisma.absence.findMany({
    where: { endedAt: null, endsAt: { lte: new Date() } },
  });
}

/** Absences ayant démarré et dont le rôle doit être attribué. */
export async function findStartedAbsences() {
  const now = new Date();
  return prisma.absence.findMany({
    where: { endedAt: null, startsAt: { lte: now }, endsAt: { gt: now } },
  });
}

export async function applyAbsenceRole(guild: Guild, userId: string) {
  return updateAbsenceRole(guild, userId, "add");
}

export async function clearAbsenceRole(guild: Guild, userId: string) {
  return updateAbsenceRole(guild, userId, "remove");
}
