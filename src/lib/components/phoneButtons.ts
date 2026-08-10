import { ActionRowBuilder, ButtonBuilder, ButtonStyle } from "discord.js";
import { PHONE_NAMESPACE } from "../../modules/phones/constants.js";
import { encodeCustomId } from "../customId.js";

export function buildPhoneButtons(guildId: string) {
  return new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId(encodeCustomId(PHONE_NAMESPACE, "add", guildId))
      .setLabel("📱 Ajouter / modifier mon numéro")
      .setStyle(ButtonStyle.Primary),
    new ButtonBuilder()
      .setCustomId(encodeCustomId(PHONE_NAMESPACE, "remove", guildId))
      .setLabel("🗑️ Retirer mon numéro")
      .setStyle(ButtonStyle.Danger),
  );
}
