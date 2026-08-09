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

export function formatLine(transaction: TransactionWithItems): string {
  const itemCount = transaction.items.length;
  const items = itemCount > 0 ? ` — ${itemCount} objet${itemCount > 1 ? "s" : ""}` : "";
  return `• **${transaction.groupe}** · ${TRANSACTION_TYPE_LABELS[transaction.type]} · ${formatPrice(transaction.prix)}${items}`;
}

export function buildChunkedFields(name: string, lines: string[], inline: boolean) {
  if (lines.length === 0) {
    return [{ name, value: "Aucune", inline }];
  }
  
  const fields = [];
  let currentChunk = "";
  let isFirst = true;

  for (const line of lines) {
    if (currentChunk.length + line.length + 1 > 1024) {
      fields.push({ name: isFirst ? name : `${name} (suite)`, value: currentChunk, inline });
      currentChunk = line;
      isFirst = false;
    } else {
      currentChunk += (currentChunk ? "\n" : "") + line;
    }
  }
  if (currentChunk) {
    fields.push({ name: isFirst ? name : `${name} (suite)`, value: currentChunk, inline });
  }

  return fields;
}

export function buildTransactionDashboardEmbeds(
  transactions: TransactionWithItems[],
): EmbedBuilder[] {
  const byStatus = new Map<TransactionStatus, TransactionWithItems[]>();
  for (const transaction of transactions) {
    const bucket = byStatus.get(transaction.statut) ?? [];
    bucket.push(transaction);
    byStatus.set(transaction.statut, bucket);
  }

  const allFields: { name: string; value: string; inline?: boolean }[] = [];

  for (const status of ACTIVE_STATUSES) {
    const bucket = byStatus.get(status) ?? [];
    const lines = bucket.map(formatLine);
    allFields.push(...buildChunkedFields(`${TRANSACTION_STATUS_EMOJIS[status]} ${TRANSACTION_STATUS_LABELS[status]} (${bucket.length})`, lines, false));
  }

  for (const status of TERMINAL_STATUSES) {
    const bucket = byStatus.get(status) ?? [];
    const lines = bucket.map(formatLine);
    allFields.push(...buildChunkedFields(`${TRANSACTION_STATUS_EMOJIS[status]} ${TRANSACTION_STATUS_LABELS[status]} (${bucket.length})`, lines, true));
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

  const embeds: EmbedBuilder[] = [];
  let currentEmbed = new EmbedBuilder().setTitle("📊 Transactions").setColor(0x5865f2);
  let currentLength = currentEmbed.data.title!.length;
  let currentFieldsCount = 0;

  for (const field of allFields) {
    const fieldLength = field.name.length + field.value.length;
    if (currentFieldsCount >= 25 || currentLength + fieldLength > 5500) {
      embeds.push(currentEmbed);
      currentEmbed = new EmbedBuilder().setColor(0x5865f2);
      currentLength = 0;
      currentFieldsCount = 0;
    }
    currentEmbed.addFields(field);
    currentLength += fieldLength;
    currentFieldsCount++;
  }

  currentEmbed.setFooter({
    text: `${activeCount} transaction(s) en cours · ${formatPrice(activeValue)} engagé(s) · ${transactions.length} au total`,
  }).setTimestamp();

  embeds.push(currentEmbed);
  return embeds;
}
