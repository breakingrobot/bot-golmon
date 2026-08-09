import { EmbedBuilder } from "discord.js";
import type { Presence, PresenceResponse } from "@prisma/client";
import { ResponseStatus } from "@prisma/client";
import { PRESENCE_COLOR_ACTIVE, PRESENCE_COLOR_CLOSED } from "../../modules/presences/constants.js";
import type { PresenceRoster } from "../../modules/presences/service.js";
import { formatMentionList, truncateField, chunkMentionsList } from "../format.js";

export type PresenceWithResponses = Presence & { responses: (PresenceResponse & { changes: any[] })[] };

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

  const changedIds = new Set(
    presence.responses.filter((r) => r.changes.some((c) => c.fromStatus !== null)).map((r) => r.userId),
  );

  const getDetail = (userId: string, baseDetail?: string) => {
    const parts = [];
    if (changedIds.has(userId)) parts.push("🔄");
    if (baseDetail) parts.push(baseDetail);
    return parts.length > 0 ? parts.join(" ") : undefined;
  };

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
    .setColor(presence.closedAt ? PRESENCE_COLOR_CLOSED : PRESENCE_COLOR_ACTIVE);

  const addChunkedFields = (
    title: string,
    userIds: string[],
    detailByUserId?: Map<string, string>,
  ) => {
    if (userIds.length === 0) {
      embed.addFields({ name: `${title} (0)`, value: "Aucun", inline: true });
      return;
    }

    const chunks = chunkMentionsList(userIds, 15);
    for (let i = 0; i < chunks.length; i++) {
      embed.addFields({
        name: i === 0 ? `${title} (${userIds.length})` : "​", // ZWSP
        value: formatMentionList(chunks[i], { detailByUserId }),
        inline: true,
      });
    }
  };

  addChunkedFields("✅ Présents", presents.map((r) => r.userId), new Map(presents.map((r) => [r.userId, getDetail(r.userId)] as const).filter((x): x is [string, string] => x[1] !== undefined)));
  addChunkedFields("❌ Absents", absents.map((r) => r.userId), new Map(absents.map((r) => [r.userId, getDetail(r.userId)] as const).filter((x): x is [string, string] => x[1] !== undefined)));
  addChunkedFields("⏳ Retards", retards.map((r) => r.userId), new Map(retards.map((r) => [r.userId, getDetail(r.userId, retardDetails.get(r.userId))] as const).filter((x): x is [string, string] => x[1] !== undefined)));

  if (nonRespondentIds.length === 0) {
    embed.addFields({
      name: "🕓 Sans réponse (0)",
      value: "Tout le monde a répondu",
    });
  } else {
    addChunkedFields("🕓 Sans réponse", nonRespondentIds);
  }

  embed.addFields(
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
