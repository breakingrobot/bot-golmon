import { EmbedBuilder } from "discord.js";
import type { Transaction, TransactionItem } from "@prisma/client";
import {
  TRANSACTION_STATUS_COLORS,
  TRANSACTION_STATUS_EMOJIS,
  TRANSACTION_STATUS_LABELS,
  TRANSACTION_TYPE_LABELS,
  formatPrice,
} from "../../modules/transactions/constants.js";
import { truncateField } from "../format.js";

export type TransactionWithItems = Transaction & { items: TransactionItem[] };

export function buildTransactionEmbed(transaction: TransactionWithItems): EmbedBuilder {
  const itemsText =
    transaction.items.length > 0
      ? transaction.items
          .map((item) => {
            const unitPrice = item.prixUnitaire ? ` — ${formatPrice(item.prixUnitaire)}` : "";
            return `• ${item.nom} ×${item.quantite}${unitPrice}`;
          })
          .join("\n")
      : "Aucun objet";

  return new EmbedBuilder()
    .setTitle(`Transaction — ${transaction.groupe}`)
    .setColor(TRANSACTION_STATUS_COLORS[transaction.statut])
    .addFields(
      { name: "Type", value: TRANSACTION_TYPE_LABELS[transaction.type], inline: true },
      { name: "Prix", value: formatPrice(transaction.prix), inline: true },
      {
        name: "Statut",
        value: `${TRANSACTION_STATUS_EMOJIS[transaction.statut]} ${TRANSACTION_STATUS_LABELS[transaction.statut]}`,
        inline: true,
      },
      { name: `Objets (${transaction.items.length})`, value: truncateField(itemsText) },
      { name: "Créée par", value: `<@${transaction.createdById}>`, inline: true },
    )
    .setFooter({ text: transaction.id })
    .setTimestamp(transaction.updatedAt);
}
