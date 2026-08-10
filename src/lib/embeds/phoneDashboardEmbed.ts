import { EmbedBuilder } from "discord.js";
import type { PhoneNumber } from "@prisma/client";
import { formatPhone } from "../../modules/phones/constants.js";
import { mention, packFields, type EmbedField } from "../format.js";

/** Titre, pied de page et marges de l'API : on ne remplit pas les 6000 caractères. */
const FIELD_BUDGET = 5200;
/** Discord plafonne à 25 champs : on en garde un pour le résumé « … ». */
const MAX_FIELDS = 24;
const FIELD_LIMIT = 1024;

/**
 * Regroupe les lignes en champs de 1024 caractères maximum, sans dépasser la
 * limite d'un champ d'embed. Chaque groupe est un tableau de lignes.
 */
function groupLines(lines: string[]): string[][] {
  return lines.reduce<string[][]>((groups, line) => {
    const current = groups.at(-1);
    const length = current ? current.join("\n").length + line.length + 1 : 0;
    if (!current || length > FIELD_LIMIT) {
      groups.push([line]);
      return groups;
    }
    current.push(line);
    return groups;
  }, []);
}

export function buildPhoneDashboardEmbed(numbers: PhoneNumber[]): EmbedBuilder[] {
  const embed = new EmbedBuilder().setTitle("📱 Annuaire téléphonique").setColor(0x5865f2).setTimestamp();

  if (numbers.length === 0) {
    embed.setDescription("Aucun numéro enregistré. Ajoute le tien avec le bouton ci-dessous ou `/numero ajouter`.");
    return [embed];
  }

  const groups = groupLines(
    numbers.map((entry) => `\`${formatPhone(entry.numero)}\` — ${mention(entry.userId)}`),
  );

  const fields: EmbedField[] = groups.map((group, index) => ({
    name: groups.length > 1 ? `Numéros (${index + 1}/${groups.length})` : "Numéros",
    value: group.join("\n"),
  }));

  // Borner le nombre de champs ne suffit pas : 24 champs pleins dépassent
  // largement la limite de 6000 caractères d'un embed, et l'edit échouerait.
  const { kept, dropped } = packFields(fields, { budget: FIELD_BUDGET, maxFields: MAX_FIELDS });
  embed.addFields(kept);

  if (dropped.length > 0) {
    const hidden = dropped.reduce((total, field) => total + field.value.split("\n").length, 0);
    embed.addFields({ name: "…", value: `+${hidden} numéro(s) non affiché(s) — utilise \`/numero voir\`.` });
  }

  embed.setFooter({ text: `${numbers.length} numéro(s) enregistré(s)` });

  return [embed];
}
