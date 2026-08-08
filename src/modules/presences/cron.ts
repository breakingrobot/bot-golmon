import cron from "node-cron";
import type { Client, TextChannel } from "discord.js";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { closePresence, getNonRespondentIds, syncPresenceMessage } from "./service.js";

async function runClosureSweep(client: Client) {
  const toClose = await prisma.presence.findMany({
    where: { closedAt: null, eventDate: { lte: new Date() } },
  });

  for (const presence of toClose) {
    await closePresence(presence.id);
    await syncPresenceMessage(client, presence.id);
    logger.info(`Présence ${presence.id} clôturée automatiquement`);
  }
}

async function runReminderSweep(client: Client) {
  const activePresences = await prisma.presence.findMany({
    where: { closedAt: null, eventDate: { gt: new Date() } },
  });

  for (const presence of activePresences) {
    try {
      const guild = await client.guilds.fetch(presence.guildId);
      const nonRespondentIds = await getNonRespondentIds(guild, presence);
      if (nonRespondentIds.length === 0) {
        continue;
      }

      const config = await prisma.presenceConfig.findUnique({ where: { guildId: presence.guildId } });
      const targetChannelId = config?.reminderChannelId ?? presence.channelId;
      const channel = await client.channels.fetch(targetChannelId);
      if (!channel?.isTextBased()) {
        continue;
      }

      await (channel as TextChannel).send({
        content: `Rappel — présence "${presence.titre}" : ${nonRespondentIds
          .map((userId) => `<@${userId}>`)
          .join(" ")}`,
        allowedMentions: { parse: ["users"] },
      });

      await prisma.presence.update({ where: { id: presence.id }, data: { lastReminderAt: new Date() } });
    } catch (error) {
      logger.error(`Erreur lors du rappel pour la présence ${presence.id}`, error);
    }
  }
}

export function startPresenceCron(client: Client): void {
  cron.schedule("0 */3 * * *", () => {
    void runClosureSweep(client);
    void runReminderSweep(client);
  });

  logger.info("Cron de rappel des présences démarré (toutes les 3h)");
}
