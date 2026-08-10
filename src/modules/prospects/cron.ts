import cron from "node-cron";
import type { Client } from "discord.js";
import { TIMEZONE } from "../../lib/datetime.js";
import { fetchGuildSafe } from "../../lib/discordClient.js";
import { logger } from "../../lib/logger.js";
import { ensureCurrentSession, syncProspectDashboard } from "./service.js";

/**
 * Bascule chaque guilde sur la semaine en cours. `ensureCurrentSession` est
 * idempotent : rejouer la routine ne crée pas de doublon, ce qui permet un
 * filet quotidien au cas où le bot serait hors ligne le lundi à minuit.
 */
async function rolloverSweep(client: Client) {
  for (const guildId of client.guilds.cache.keys()) {
    try {
      const guild = await fetchGuildSafe(client, guildId, `la guilde ${guildId}`);
      if (!guild) {
        continue;
      }
      await ensureCurrentSession(guild);
      await syncProspectDashboard(client, guildId);
    } catch (error) {
      logger.error(`Bascule hebdomadaire des prospects échouée pour ${guildId}`, error);
    }
  }
}

export function startProspectCron(client: Client): void {
  // Bascule hebdomadaire (lundi 00:00) et filet quotidien de rattrapage.
  cron.schedule("0 0 * * 1", () => void rolloverSweep(client), { timezone: TIMEZONE });
  cron.schedule("5 0 * * *", () => void rolloverSweep(client), { timezone: TIMEZONE });

  logger.info("Cron des prospects démarré (reset hebdomadaire le lundi, rattrapage quotidien)");
}
