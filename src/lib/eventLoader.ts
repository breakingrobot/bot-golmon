import { readdirSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import type { Client } from "discord.js";
import type { BotEvent } from "../types/event.js";
import { logger } from "./logger.js";

export async function loadEvents(client: Client, eventsDir: string): Promise<void> {
  const files = readdirSync(eventsDir, { withFileTypes: true }).filter(
    (entry) => entry.isFile() && (entry.name.endsWith(".js") || entry.name.endsWith(".ts")),
  );

  let count = 0;

  for (const file of files) {
    const fullPath = path.join(eventsDir, file.name);
    const imported = (await import(pathToFileURL(fullPath).href)) as { event?: BotEvent };
    const event = imported.event;

    if (!event) {
      logger.warn(`Fichier d'event sans export "event": ${fullPath}`);
      continue;
    }

    if (event.once) {
      client.once(event.name, (...args) => void event.execute(...args));
    } else {
      client.on(event.name, (...args) => void event.execute(...args));
    }

    count++;
  }

  logger.info(`${count} event(s) chargé(s)`);
}
