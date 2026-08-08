import { Client, Collection, GatewayIntentBits } from "discord.js";
import type { Command } from "../types/command.js";

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
