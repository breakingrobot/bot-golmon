import { EmbedBuilder } from "discord.js";
import type { Presence, PresenceResponse } from "@prisma/client";
import { ResponseStatus } from "@prisma/client";
import { PRESENCE_COLOR_ACTIVE, PRESENCE_COLOR_CLOSED } from "../../modules/presences/constants.js";

export type PresenceWithResponses = Presence & { responses: PresenceResponse[] };

function formatMentionList(userIds: string[], detailByUserId?: Map<string, string>): string {
  if (userIds.length === 0) {
    return "Aucun";
  }
  return userIds
    .map((id) => `<@${id}>${detailByUserId?.get(id) ? ` (${detailByUserId.get(id)})` : ""}`)
    .join("\n");
}

function buildPercentBar(percent: number): string {
  const filled = Math.round(percent / 10);
  return "🟩".repeat(filled) + "⬜".repeat(10 - filled);
}

export function buildPresenceEmbed(
  presence: PresenceWithResponses,
  guildIconUrl: string | null,
  effectifTotal: number,
): EmbedBuilder {
  const presents = presence.responses.filter((r) => r.status === ResponseStatus.PRESENT);
  const absents = presence.responses.filter((r) => r.status === ResponseStatus.ABSENT);
  const retards = presence.responses.filter((r) => r.status === ResponseStatus.RETARD);

  const retardDetails = new Map(
    retards
      .filter((r) => r.retardMinutes != null)
      .map((r) => [r.userId, `${r.retardMinutes} min`]),
  );

  const nbReponses = presence.responses.length;
  const percent = effectifTotal > 0 ? Math.round((presents.length / effectifTotal) * 100) : 0;

  const dateLabel = `<t:${Math.floor(presence.eventDate.getTime() / 1000)}:f>`;

  const embed = new EmbedBuilder()
    .setAuthor({ name: "Présence" })
    .setTitle(presence.titre)
    .setDescription(
      `Présence pour le ${dateLabel}${presence.description ? `\n\n${presence.description}` : ""}\n\n${buildPercentBar(percent)} ${percent}%`,
    )
    .setColor(presence.closedAt ? PRESENCE_COLOR_CLOSED : PRESENCE_COLOR_ACTIVE)
    .addFields(
      { name: "✅ Présents (" + presents.length + ")", value: formatMentionList(presents.map((r) => r.userId)), inline: true },
      { name: "❌ Absents (" + absents.length + ")", value: formatMentionList(absents.map((r) => r.userId)), inline: true },
      { name: "⏳ Retards (" + retards.length + ")", value: formatMentionList(retards.map((r) => r.userId), retardDetails), inline: true },
      { name: "👥 Effectif", value: `${nbReponses}/${effectifTotal}`, inline: true },
      { name: "📣 Émetteur de la présence", value: `<@${presence.createdById}>`, inline: true },
      { name: "📅 Date d'émission", value: `<t:${Math.floor(presence.createdAt.getTime() / 1000)}:f>`, inline: true },
    )
    .setFooter({ text: presence.id })
    .setTimestamp();

  if (guildIconUrl) {
    embed.setThumbnail(guildIconUrl);
  }

  return embed;
}
