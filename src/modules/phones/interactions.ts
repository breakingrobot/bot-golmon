import {
  ActionRowBuilder,
  MessageFlags,
  ModalBuilder,
  TextInputBuilder,
  TextInputStyle,
  type ButtonInteraction,
  type ModalSubmitInteraction,
} from "discord.js";
import { encodeCustomId } from "../../lib/customId.js";
import { GUILD_ONLY } from "../../lib/replies.js";
import {
  formatPhone,
  INVALID_FORMAT,
  normalizePhone,
  PHONE_NAMESPACE,
  PHONE_REGEX,
} from "./constants.js";
import { NUMBER_TAKEN, removePhoneNumber, setPhoneNumber, syncPhoneDashboard } from "./service.js";

function buildNumeroModal(guildId: string): ModalBuilder {
  return new ModalBuilder()
    .setCustomId(encodeCustomId(PHONE_NAMESPACE, "add-modal", guildId))
    .setTitle("Ajouter un numéro")
    .addComponents(
      new ActionRowBuilder<TextInputBuilder>().addComponents(
        new TextInputBuilder()
          .setCustomId("numero")
          .setLabel("Numéro de téléphone")
          .setPlaceholder("5551234")
          .setStyle(TextInputStyle.Short)
          .setRequired(true),
      ),
    );
}

export async function handlePhoneButton(interaction: ButtonInteraction, action: string, id: string) {
  if (action === "add") {
    await interaction.showModal(buildNumeroModal(id));
    return;
  }

  if (action !== "remove") {
    return;
  }

  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  const removed = await removePhoneNumber(interaction.guildId, interaction.user.id);
  if (!removed) {
    await interaction.editReply("Tu n'as pas de numéro enregistré.");
    return;
  }
  await syncPhoneDashboard(interaction.client, interaction.guildId);
  await interaction.editReply("Ton numéro a été retiré.");
}

export async function handlePhoneModal(interaction: ModalSubmitInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const numero = normalizePhone(interaction.fields.getTextInputValue("numero"));
  if (!PHONE_REGEX.test(numero)) {
    await interaction.reply({ content: INVALID_FORMAT, flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.deferUpdate();

  const result = await setPhoneNumber(interaction.guildId, interaction.user.id, numero);
  if (result.status === "taken") {
    await interaction.followUp({ content: NUMBER_TAKEN, flags: MessageFlags.Ephemeral });
    return;
  }

  await syncPhoneDashboard(interaction.client, interaction.guildId);
  await interaction.followUp({
    content: `Numéro enregistré : \`${formatPhone(numero)}\``,
    flags: MessageFlags.Ephemeral,
  });
}
