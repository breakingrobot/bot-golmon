import { ActionRowBuilder, ButtonBuilder, ButtonStyle } from "discord.js";
import { encodeCustomId } from "../customId.js";

export function buildPresenceButtons(
  presenceId: string,
  counts: { present: number; absent: number; retard: number },
  disabled: boolean,
): ActionRowBuilder<ButtonBuilder> {
  return new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId(encodeCustomId("presence", "present", presenceId))
      .setLabel(`Présent (${counts.present})`)
      .setEmoji("✅")
      .setStyle(ButtonStyle.Success)
      .setDisabled(disabled),
    new ButtonBuilder()
      .setCustomId(encodeCustomId("presence", "absent", presenceId))
      .setLabel(`Absent (${counts.absent})`)
      .setEmoji("❌")
      .setStyle(ButtonStyle.Danger)
      .setDisabled(disabled),
    new ButtonBuilder()
      .setCustomId(encodeCustomId("presence", "retard", presenceId))
      .setLabel(`Retard (${counts.retard})`)
      .setEmoji("⏳")
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(disabled),
    new ButtonBuilder()
      .setCustomId(encodeCustomId("presence", "ping", presenceId))
      .setLabel("Ping")
      .setEmoji("📌")
      .setStyle(ButtonStyle.Primary)
      .setDisabled(disabled),
  );
}
