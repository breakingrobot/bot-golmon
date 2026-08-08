import { ChannelType, SlashCommandBuilder } from "discord.js";
import type { TransactionType } from "@prisma/client";
import type { Command } from "../../types/command.js";
import { TRANSACTION_TYPE_CHOICES } from "../../modules/transactions/constants.js";
import { createTransaction, syncTransactionMessage } from "../../modules/transactions/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction-create")
    .setDescription("Crée une nouvelle transaction")
    .addStringOption((option) =>
      option.setName("groupe").setDescription("Groupe concerné").setRequired(true),
    )
    .addStringOption((option) =>
      option
        .setName("type")
        .setDescription("Achat ou vente")
        .setRequired(true)
        .addChoices(...TRANSACTION_TYPE_CHOICES),
    )
    .addNumberOption((option) =>
      option.setName("prix").setDescription("Prix total").setRequired(true),
    )
    .addChannelOption((option) =>
      option
        .setName("channel")
        .setDescription("Salon d'affichage (par défaut: salon actuel)")
        .addChannelTypes(ChannelType.GuildText),
    ),
  async execute(interaction) {
    if (!interaction.guildId) {
      await interaction.reply({ content: "Commande utilisable en serveur uniquement.", ephemeral: true });
      return;
    }

    const groupe = interaction.options.getString("groupe", true);
    const type = interaction.options.getString("type", true) as TransactionType;
    const prix = interaction.options.getNumber("prix", true);
    const channel = interaction.options.getChannel("channel") ?? interaction.channel;

    if (!channel) {
      await interaction.reply({ content: "Salon introuvable.", ephemeral: true });
      return;
    }

    const transaction = await createTransaction({
      guildId: interaction.guildId,
      channelId: channel.id,
      groupe,
      type,
      prix,
      createdById: interaction.user.id,
    });

    await syncTransactionMessage(interaction.client, transaction.id);

    await interaction.reply({
      content: `Transaction créée : \`${transaction.id}\``,
      ephemeral: true,
    });
  },
};
