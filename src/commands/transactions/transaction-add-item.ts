import { SlashCommandBuilder } from "discord.js";
import type { Command } from "../../types/command.js";
import { addTransactionItem, findOpenTransactions, syncTransactionMessage } from "../../modules/transactions/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction-add-item")
    .setDescription("Ajoute un objet à une transaction")
    .addStringOption((option) =>
      option
        .setName("transaction_id")
        .setDescription("Transaction concernée")
        .setRequired(true)
        .setAutocomplete(true),
    )
    .addStringOption((option) => option.setName("nom").setDescription("Nom de l'objet").setRequired(true))
    .addIntegerOption((option) =>
      option.setName("quantite").setDescription("Quantité (défaut: 1)").setMinValue(1),
    )
    .addNumberOption((option) => option.setName("prix_unitaire").setDescription("Prix unitaire")),
  async execute(interaction) {
    const transactionId = interaction.options.getString("transaction_id", true);
    const nom = interaction.options.getString("nom", true);
    const quantite = interaction.options.getInteger("quantite") ?? 1;
    const prixUnitaire = interaction.options.getNumber("prix_unitaire") ?? undefined;

    await addTransactionItem(transactionId, { nom, quantite, prixUnitaire });
    await syncTransactionMessage(interaction.client, transactionId);

    await interaction.reply({ content: `Objet "${nom}" ajouté.`, ephemeral: true });
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
