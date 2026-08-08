import { SlashCommandBuilder } from "discord.js";
import type { Command } from "../../types/command.js";
import { getTransaction, findOpenTransactions } from "../../modules/transactions/service.js";
import { buildTransactionEmbed } from "../../lib/embeds/transactionEmbed.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction-view")
    .setDescription("Affiche une transaction")
    .addStringOption((option) =>
      option
        .setName("transaction_id")
        .setDescription("Transaction concernée")
        .setRequired(true)
        .setAutocomplete(true),
    ),
  async execute(interaction) {
    const transactionId = interaction.options.getString("transaction_id", true);
    const transaction = await getTransaction(transactionId);

    if (!transaction) {
      await interaction.reply({ content: "Transaction introuvable.", ephemeral: true });
      return;
    }

    await interaction.reply({ embeds: [buildTransactionEmbed(transaction)], ephemeral: true });
  },
  async autocomplete(interaction) {
    if (!interaction.guildId) {
      await interaction.respond([]);
      return;
    }
    const query = interaction.options.getFocused();
    const transactions = await findOpenTransactions(interaction.guildId, query);
    await interaction.respond(
      transactions.map((t) => ({ name: `${t.groupe} — ${t.id}`, value: t.id })),
    );
  },
};
