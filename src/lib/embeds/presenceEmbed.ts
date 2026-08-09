import { EmbedBuilder } from "discord.js";
import type { Presence, PresenceResponse } from "@prisma/client";
import { ResponseStatus } from "@prisma/client";
import { PRESENCE_COLOR_ACTIVE, PRESENCE_COLOR_CLOSED } from "../../modules/presences/constants.js";
import type { PresenceRoster } from "../../modules/presences/service.js";
import { formatMentionList, truncateField } from "../format.js";

export type PresenceWithResponses = Presence & { responses: PresenceResponse[] };

function buildPercentBar(percent: number): string {
  const filled = Math.min(10, Math.max(0, Math.round(percent / 10)));
  return "🟩".repeat(filled) + "⬜".repeat(10 - filled);
}

function unix(date: Date): number {
  return Math.floor(date.getTime() / 1000);
}

export function buildPresenceEmbed(
  presence: PresenceWithResponses,
  guildIconUrl: string | null,
  roster: PresenceRoster,
): EmbedBuilder {
  const presents = presence.responses.filter((r) => r.status === ResponseStatus.PRESENT);
  const absents = presence.responses.filter((r) => r.status === ResponseStatus.ABSENT);
  const retards = presence.responses.filter((r) => r.status === ResponseStatus.RETARD);

  const retardDetails = new Map(
    retards.filter((r) => r.retardMinutes != null).map((r) => [r.userId, `${r.retardMinutes} min`]),
  );

  const { effectifTotal, nonRespondentIds } = roster;
  const nbReponses = presence.responses.length;
  const percent = effectifTotal > 0 ? Math.round((presents.length / effectifTotal) * 100) : 0;

  const header = `Présence pour le <t:${unix(presence.eventDate)}:F>`;
  const description = presence.description ? `\n\n${presence.description}` : "";
  const closed = presence.closedAt ? "\n\n🔒 **Présence clôturée**" : "";

  const embed = new EmbedBuilder()
    .setAuthor({ name: "Présence" })
    .setTitle(presence.titre)
    .setDescription(
      truncateField(`${header}${description}\n\n${buildPercentBar(percent)} ${percent}%${closed}`, 4096),
    )
    .setColor(presence.closedAt ? PRESENCE_COLOR_CLOSED : PRESENCE_COLOR_ACTIVE)
    .addFields(
      {
        name: `✅ Présents (${presents.length})`,
        value: formatMentionList(presents.map((r) => r.userId)),
        inline: true,
      },
      {
        name: `❌ Absents (${absents.length})`,
        value: formatMentionList(absents.map((r) => r.userId)),
        inline: true,
      },
      {
        name: `⏳ Retards (${retards.length})`,
        value: formatMentionList(retards.map((r) => r.userId), { detailByUserId: retardDetails }),
        inline: true,
      },
      {
        name: `🕓 Sans réponse (${nonRespondentIds.length})`,
        value: formatMentionList(nonRespondentIds, { empty: "Tout le monde a répondu" }),
      },
      { name: "👥 Effectif", value: `${nbReponses}/${effectifTotal}`, inline: true },
      { name: "📣 Émetteur", value: `<@${presence.createdById}>`, inline: true },
      { name: "📅 Date d'émission", value: `<t:${unix(presence.createdAt)}:f>`, inline: true },
    )
    .setFooter({ text: presence.id })
    .setTimestamp();

  if (guildIconUrl) {
    embed.setThumbnail(guildIconUrl);
  }

  return embed;
}
