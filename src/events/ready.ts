import type { Client } from "discord.js";
import type { BotEvent } from "../types/event.js";
import { isGuildAllowed } from "../lib/guildWhitelist.js";
import { logger } from "../lib/logger.js";

async function leaveUnauthorizedGuilds(client: Client<true>) {
  for (const guild of client.guilds.cache.values()) {
    if (!isGuildAllowed(guild.id)) {
      logger.warn(`Guilde non autorisée ${guild.name} (${guild.id}) — départ`);
      await guild.leave();
    }
  }
}

export const event: BotEvent<"clientReady"> = {
  name: "clientReady",
  once: true,
  async execute(client: Client<true>) {
    logger.info(`Connecté en tant que ${client.user.tag}`);
    await leaveUnauthorizedGuilds(client);
  },
};
