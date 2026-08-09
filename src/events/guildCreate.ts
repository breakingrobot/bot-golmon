import type { BotEvent } from "../types/event.js";
import { isGuildAllowed } from "../lib/guildWhitelist.js";
import { logger } from "../lib/logger.js";

export const event: BotEvent<"guildCreate"> = {
  name: "guildCreate",
  async execute(guild) {
    if (isGuildAllowed(guild.id)) {
      logger.info(`Rejoint la guilde autorisée ${guild.name} (${guild.id})`);
      return;
    }

    logger.warn(`Guilde non autorisée ${guild.name} (${guild.id}) — départ immédiat`);
    await guild.leave();
  },
};
