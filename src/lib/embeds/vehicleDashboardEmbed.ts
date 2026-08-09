import { EmbedBuilder } from "discord.js";
import type { Vehicle } from "@prisma/client";
import { truncateField } from "../format.js";

/** Discord plafonne à 25 champs par embed : on en garde un pour le résumé. */
const MAX_OWNER_FIELDS = 24;

function formatVehicleLine(vehicle: Vehicle): string {
  return vehicle.modele ? `\`${vehicle.plate}\` — ${vehicle.modele}` : `\`${vehicle.plate}\``;
}

export function buildVehicleDashboardEmbed(vehicles: Vehicle[]): EmbedBuilder {
  const embed = new EmbedBuilder().setTitle("🚗 Véhicules").setColor(0x5865f2).setTimestamp();

  if (vehicles.length === 0) {
    embed.setDescription("Aucun véhicule enregistré. Ajoute-en un avec `/vehicle add`.");
    return embed;
  }

  const byOwner = new Map<string, Vehicle[]>();
  for (const vehicle of vehicles) {
    const bucket = byOwner.get(vehicle.ownerId) ?? [];
    bucket.push(vehicle);
    byOwner.set(vehicle.ownerId, bucket);
  }

  // Propriétaires les mieux fournis en premier.
  const owners = [...byOwner.entries()].toSorted((a, b) => b[1].length - a[1].length);
  const shown = owners.slice(0, MAX_OWNER_FIELDS);

  for (const [ownerId, owned] of shown) {
    const isRole = owned.length > 0 && owned[0].ownerIsRole;
    const mention = isRole ? `<@&${ownerId}>` : `<@${ownerId}>`;
    embed.addFields({
      name: `${owned.length} véhicule${owned.length > 1 ? "s" : ""}`,
      value: truncateField(`${mention}\n${owned.map(formatVehicleLine).join("\n")}`),
      inline: true,
    });
  }

  const hiddenOwners = owners.slice(MAX_OWNER_FIELDS);
  if (hiddenOwners.length > 0) {
    const hiddenVehicles = hiddenOwners.reduce((total, [, owned]) => total + owned.length, 0);
    embed.addFields({
      name: "…",
      value: `+${hiddenOwners.length} autre(s) propriétaire(s), ${hiddenVehicles} véhicule(s)`,
    });
  }

  embed.setFooter({
    text: `${vehicles.length} véhicule(s) · ${owners.length} propriétaire(s)`,
  });

  return embed;
}
