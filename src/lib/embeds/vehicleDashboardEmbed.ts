import { EmbedBuilder } from "discord.js";
import type { Vehicle } from "@prisma/client";
import { ownerMention, packFields, truncateField, type EmbedField } from "../format.js";

/** Titre, pied de page et marges de l'API : on ne remplit pas les 6000 caractères. */
const FIELD_BUDGET = 5200;
/** Discord plafonne à 25 champs : on en garde un pour le résumé « … ». */
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

  const fields: EmbedField[] = owners.map(([ownerId, owned]) => ({
    name: `${owned.length} véhicule${owned.length > 1 ? "s" : ""}`,
    value: truncateField(
      `${ownerMention(ownerId, owned[0].ownerIsRole)}\n${owned.map(formatVehicleLine).join("\n")}`,
    ),
    inline: true,
  }));

  // Borner le nombre de champs ne suffit pas : 24 champs pleins font quatre fois
  // la limite de 6000 caractères d'un embed, et l'edit échouerait.
  const { kept, dropped } = packFields(fields, {
    budget: FIELD_BUDGET,
    maxFields: MAX_OWNER_FIELDS,
  });
  embed.addFields(kept);

  if (dropped.length > 0) {
    const hiddenVehicles = owners
      .slice(kept.length)
      .reduce((total, [, owned]) => total + owned.length, 0);
    embed.addFields({
      name: "…",
      value: `+${dropped.length} autre(s) propriétaire(s), ${hiddenVehicles} véhicule(s)`,
    });
  }

  embed.setFooter({
    text: `${vehicles.length} véhicule(s) · ${owners.length} propriétaire(s)`,
  });

  return embed;
}
