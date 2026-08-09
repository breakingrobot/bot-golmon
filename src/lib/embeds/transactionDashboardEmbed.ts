import { EmbedBuilder } from "discord.js";
import { TransactionStatus } from "@prisma/client";
import type { TransactionWithItems } from "./transactionEmbed.js";
import {
  ACTIVE_STATUSES,
  TERMINAL_STATUSES,
  TRANSACTION_STATUS_EMOJIS,
  TRANSACTION_STATUS_LABELS,
  TRANSACTION_TYPE_LABELS,
  formatPrice,
} from "../../modules/transactions/constants.js";
import { truncateField } from "../format.js";

const TERMINAL_PREVIEW = 3;

function formatLine(transaction: TransactionWithItems): string {
  const itemCount = transaction.items.length;
  const items = itemCount > 0 ? ` — ${itemCount} objet${itemCount > 1 ? "s" : ""}` : "";
  return `• **${transaction.groupe}** · ${TRANSACTION_TYPE_LABELS[transaction.type]} · ${formatPrice(transaction.prix)}${items}`;
}

function buildSection(transactions: TransactionWithItems[], limit?: number): string {
  if (transactions.length === 0) {
    return "Aucune";
  }

  const shown = limit ? transactions.slice(0, limit) : transactions;
  const lines = shown.map(formatLine);
  const remaining = transactions.length - shown.length;
  const text = remaining > 0 ? `${lines.join("\n")}\n… +${remaining} autre${remaining > 1 ? "s" : ""}` : lines.join("\n");

  return truncateField(text);
}

export function buildTransactionDashboardEmbed(
  transactions: TransactionWithItems[],
): EmbedBuilder {
  const byStatus = new Map<TransactionStatus, TransactionWithItems[]>();
  for (const transaction of transactions) {
    const bucket = byStatus.get(transaction.statut) ?? [];
    bucket.push(transaction);
    byStatus.set(transaction.statut, bucket);
  }

  const embed = new EmbedBuilder()
    .setTitle("📊 Transactions")
    .setColor(0x5865f2)
    .setTimestamp();

  for (const status of ACTIVE_STATUSES) {
    const bucket = byStatus.get(status) ?? [];
    embed.addFields({
      name: `${TRANSACTION_STATUS_EMOJIS[status]} ${TRANSACTION_STATUS_LABELS[status]} (${bucket.length})`,
      value: buildSection(bucket),
    });
  }

  for (const status of TERMINAL_STATUSES) {
    const bucket = byStatus.get(status) ?? [];
    embed.addFields({
      name: `${TRANSACTION_STATUS_EMOJIS[status]} ${TRANSACTION_STATUS_LABELS[status]} (${bucket.length})`,
      value: buildSection(bucket, TERMINAL_PREVIEW),
      inline: true,
    });
  }

  const activeCount = ACTIVE_STATUSES.reduce(
    (total, status) => total + (byStatus.get(status)?.length ?? 0),
    0,
  );
  const activeValue = ACTIVE_STATUSES.reduce(
    (total, status) =>
      total + (byStatus.get(status) ?? []).reduce((sum, item) => sum + item.prix, 0),
    0,
  );

  embed.setFooter({
    text: `${activeCount} transaction(s) en cours · ${formatPrice(activeValue)} engagé(s) · ${transactions.length} au total`,
  });

  return embed;
}
