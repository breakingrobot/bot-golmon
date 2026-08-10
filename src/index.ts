import path from "node:path";
import { fileURLToPath } from "node:url";
import "dotenv/config";
import { createDiscordClient } from "./lib/discordClient.js";
import { loadCommands } from "./lib/commandLoader.js";
import { loadEvents } from "./lib/eventLoader.js";
import { logger } from "./lib/logger.js";
import { startPresenceCron } from "./modules/presences/cron.js";
import { startProspectCron } from "./modules/prospects/cron.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const token = process.env.DISCORD_TOKEN;
if (!token) {
  throw new Error("DISCORD_TOKEN manquant dans les variables d'environnement");
}

const client = createDiscordClient();

await loadCommands(client, path.join(__dirname, "commands"));
await loadEvents(client, path.join(__dirname, "events"));

startPresenceCron(client);
startProspectCron(client);

client.on("error", (error) => {
  logger.error("Erreur client Discord", error);
});

await client.login(token);
