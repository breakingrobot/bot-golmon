import cron from "node-cron";
import type { Client } from "discord.js";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { chunkMentions } from "../../lib/format.js";
import { TIMEZONE } from "../../lib/datetime.js";
import {
  closePresence,
  ensurePresenceAlive,
  getNonRespondentIds,
  syncPresenceMessage,
} from "./service.js";
import {
  applyAbsenceRole,
  endAbsence,
  findExpiredAbsences,
  findStartedAbsences,
} from "./absences.js";

const REMINDER_INTERVAL_MS = 3 * 60 * 60 * 1000;

/** Clôture les présences dont l'heure est passée (contrôle toutes les 5 min). */
async function runClosureSweep(client: Client) {
  const toClose = await prisma.presence.findMany({
    where: { closedAt: null, eventDate: { lte: new Date() } },
  });

  for (const presence of toClose) {
    try {
      // Message disparu ou jamais publié : la présence est supprimée, rien à clôturer.
      if (!(await ensurePresenceAlive(client, presence))) {
        continue;
      }
      await closePresence(presence.id);
      await syncPresenceMessage(client, presence.id);
      logger.info(`Présence ${presence.id} clôturée automatiquement`);
    } catch (error) {
      logger.error(`Échec de la clôture de la présence ${presence.id}`, error);
    }
  }
}

/**
 * Relance les non-répondants, au plus une fois toutes les 3h par présence.
 * `lastReminderAt` est initialisé à la création (qui ping déjà tout le monde).
 */
async function runReminderSweep(client: Client) {
  const now = new Date();
  const threshold = new Date(now.getTime() - REMINDER_INTERVAL_MS);

  const activePresences = await prisma.presence.findMany({
    where: {
      closedAt: null,
      eventDate: { gt: now },
      OR: [{ lastReminderAt: null }, { lastReminderAt: { lte: threshold } }],
    },
  });

  for (const presence of activePresences) {
    try {
      // Pas de rappel pour une présence dont le message n'existe plus :
      // personne ne pourrait y répondre. Elle est supprimée à la place.
      if (!(await ensurePresenceAlive(client, presence))) {
        continue;
      }

      const guild = await client.guilds.fetch(presence.guildId);
      const nonRespondentIds = await getNonRespondentIds(guild, presence);
      if (nonRespondentIds.length === 0) {
        continue;
      }

      const config = await prisma.presenceConfig.findUnique({
        where: { guildId: presence.guildId },
      });
      const targetChannelId = config?.reminderChannelId ?? presence.channelId;
      const channel = await client.channels.fetch(targetChannelId).catch(() => null);
      if (!channel?.isSendable()) {
        continue;
      }

      const prefix = `⏰ Rappel — **${presence.titre}** <t:${Math.floor(presence.eventDate.getTime() / 1000)}:R>`;
      for (const content of chunkMentions(nonRespondentIds, prefix)) {
        await channel.send({ content, allowedMentions: { parse: ["users"] } });
      }

      await prisma.presence.update({
        where: { id: presence.id },
        data: { lastReminderAt: now },
      });
      logger.info(`Rappel envoyé pour ${presence.id} (${nonRespondentIds.length} membre(s))`);
    } catch (error) {
      logger.error(`Erreur lors du rappel pour la présence ${presence.id}`, error);
    }
  }
}

/**
 * Applique le rôle aux absences qui viennent de commencer et le retire à celles
 * qui se terminent. Le retrait ne se fait que si le membre n'a aucune autre
 * absence active, pour gérer les périodes qui se chevauchent.
 */
async function runAbsenceSweep(client: Client) {
  for (const absence of await findStartedAbsences()) {
    try {
      const guild = await client.guilds.fetch(absence.guildId);
      await applyAbsenceRole(guild, absence.userId);
    } catch (error) {
      logger.error(`Application du rôle d'absence ${absence.id} échouée`, error);
    }
  }

  for (const absence of await findExpiredAbsences()) {
    try {
      const guild = await client.guilds.fetch(absence.guildId);
      await endAbsence(guild, absence.id, absence.userId);
      logger.info(`Absence ${absence.id} terminée automatiquement`);
    } catch (error) {
      logger.error(`Clôture de l'absence ${absence.id} échouée`, error);
    }
  }
}

export function startPresenceCron(client: Client): void {
  cron.schedule(
    "*/5 * * * *",
    async () => {
      await runClosureSweep(client);
      await runReminderSweep(client);
      await runAbsenceSweep(client);
    },
    { timezone: TIMEZONE },
  );

  logger.info("Cron des présences démarré (contrôle toutes les 5 min, rappels espacés de 3h)");
}
