import { Client, Collection, GatewayIntentBits } from "discord.js";
import type { Command } from "../types/command.js";
import { logger } from "./logger.js";

declare module "discord.js" {
  interface Client {
    commands: Collection<string, Command>;
  }
}

export function createDiscordClient(): Client {
  const client = new Client({
    intents: [
      GatewayIntentBits.Guilds,
      GatewayIntentBits.GuildMembers,
      GatewayIntentBits.GuildMessages,
    ],
  });

  client.commands = new Collection();

  return client;
}

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Salon supprimé, bot exclu, permissions retirées : autant de cas normaux qui ne
 * doivent pas faire tomber la commande en cours. `context` décrit ce qu'on
 * cherchait, pour que le log reste lisible sans pile d'appels.
 */
export async function fetchChannelSafe(client: Client, channelId: string, context: string) {
  return client.channels.fetch(channelId).catch((error: unknown) => {
    logger.warn(`Impossible de récupérer ${context} : ${reason(error)}`);
    return null;
  });
}

export async function fetchGuildSafe(client: Client, guildId: string, context: string) {
  return client.guilds.fetch(guildId).catch((error: unknown) => {
    logger.warn(`Impossible de récupérer ${context} : ${reason(error)}`);
    return null;
  });
}
