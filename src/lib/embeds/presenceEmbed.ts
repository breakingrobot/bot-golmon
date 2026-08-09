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

  let currentLength =
    (embed.data.author?.name?.length ?? 0) +
    (embed.data.title?.length ?? 0) +
    (embed.data.description?.length ?? 0) +
    150; // padding for footer and fixed fields

  let fieldsCount = 0;

  const addChunkedFields = (
    title: string,
    userIds: string[],
    detailByUserId?: Map<string, string>,
    emptyText = "Aucun",
  ) => {
    if (userIds.length === 0) {
      if (fieldsCount >= 21 || currentLength > 5500) return;
      embed.addFields({ name: `${title} (0)`, value: emptyText, inline: true });
      fieldsCount++;
      currentLength += title.length + 9 + emptyText.length;
      return;
    }

    const chunks = chunkMentionsList(userIds, 15);
    let i = 0;
    for (const chunk of chunks) {
      if (fieldsCount >= 21 || currentLength > 5000) {
        const remaining = userIds.length - (i * 15);
        if (remaining > 0) {
           const name = i === 0 ? `${title} (${userIds.length})` : "​";
           const value = `... +${remaining} autre(s)`;
           embed.addFields({ name, value, inline: true });
           fieldsCount++;
           currentLength += name.length + value.length;
        }
        break;
      }

      const name = i === 0 ? `${title} (${userIds.length})` : "​";
      const value = formatMentionList(chunk, { detailByUserId });
      embed.addFields({ name, value, inline: true });
      fieldsCount++;
      currentLength += name.length + value.length;
      i++;
    }
  };

  addChunkedFields("✅ Présents", presents.map((r) => r.userId), new Map(presents.map((r) => [r.userId, getDetail(r.userId)] as const).filter((x): x is [string, string] => x[1] !== undefined)));
  addChunkedFields("❌ Absents", absents.map((r) => r.userId), new Map(absents.map((r) => [r.userId, getDetail(r.userId)] as const).filter((x): x is [string, string] => x[1] !== undefined)));
  addChunkedFields("⏳ Retards", retards.map((r) => r.userId), new Map(retards.map((r) => [r.userId, getDetail(r.userId, retardDetails.get(r.userId))] as const).filter((x): x is [string, string] => x[1] !== undefined)));

  addChunkedFields("🕓 Sans réponse", nonRespondentIds, undefined, "Tout le monde a répondu");

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
