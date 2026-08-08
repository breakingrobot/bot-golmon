import type { Client } from "discord.js";
import type { BotEvent } from "../types/event.js";
import { logger } from "../lib/logger.js";

export const event: BotEvent<"clientReady"> = {
  name: "clientReady",
  once: true,
  async execute(client: Client<true>) {
    logger.info(`Connecté en tant que ${client.user.tag}`);
  },
};
