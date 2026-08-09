import { EmbedBuilder } from "discord.js";
import type { Vehicle } from "@prisma/client";
import type { ApiVehicle } from "../../modules/vehicles/api.js";

export function buildVehicleLookupEmbed(
  apiVehicle: ApiVehicle,
  registered: Vehicle | null,
): EmbedBuilder {
  const embed = new EmbedBuilder()
    .setTitle(`🚗 ${apiVehicle.name ?? "Véhicule"}`)
    .setColor(apiVehicle.illegal ? 0xe74c3c : 0x2ecc71)
    .addFields(
      { name: "Plaque", value: `\`${apiVehicle.plate}\``, inline: true },
      { name: "Modèle", value: apiVehicle.name ?? apiVehicle.model ?? "Inconnu", inline: true },
      { name: "Illégal", value: apiVehicle.illegal ? "⚠️ Oui" : "Non", inline: true },
    );

  // `owner` vient du jeu (joueur ou organisation) : à ne pas confondre avec le
  // propriétaire Discord enregistré dans le bot, affiché juste en dessous.
  if (apiVehicle.owner) {
    embed.addFields({
      name: "Propriétaire en jeu",
      value: apiVehicle.owner.name,
      inline: true,
    });
  }
  if (apiVehicle.buyer) {
    embed.addFields({ name: "Acheteur", value: apiVehicle.buyer.name, inline: true });
  }

  embed.addFields({
    name: "Dans le registre",
    value: registered
      ? `<@${registered.ownerId}>${registered.modele ? ` — ${registered.modele}` : ""}`
      : "Non enregistré · `/vehicle add` pour l'ajouter",
  });

  if (apiVehicle.model) {
    embed.setFooter({ text: `Code modèle : ${apiVehicle.model}` });
  }

  return embed;
}
