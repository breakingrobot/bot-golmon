import { EmbedBuilder } from "discord.js";
import { TransactionStatus } from "@prisma/client";
import type { TransactionWithItems } from "./transactionEmbed.js";
import { packFields, type EmbedField } from "../format.js";
import {
  ACTIVE_STATUSES,
  TERMINAL_STATUSES,
  TRANSACTION_STATUS_EMOJIS,
  TRANSACTION_STATUS_LABELS,
  TRANSACTION_TYPE_LABELS,
  formatPrice,
} from "../../modules/transactions/constants.js";

/** Titre, pied de page et marges de l'API : on ne remplit pas les 6000 caractères. */
const FIELD_BUDGET = 5200;
/** Un champ est réservé au résumé « … +N » en cas de débordement. */
const MAX_FIELDS = 24;

export function formatLine(transaction: TransactionWithItems): string {
  const itemCount = transaction.items.length;
  const items = itemCount > 0 ? ` — ${itemCount} objet${itemCount > 1 ? "s" : ""}` : "";
  return `• **${transaction.groupe}** · ${TRANSACTION_TYPE_LABELS[transaction.type]} · ${formatPrice(transaction.prix)}${items}`;
}

export function buildChunkedFields(name: string, lines: string[], inline: boolean): EmbedField[] {
  if (lines.length === 0) {
    return [{ name, value: "Aucune", inline }];
  }

  const result = lines.reduce(
    (acc, line) => {
      const current = acc.chunks[acc.chunks.length - 1];
      if (!current) {
        acc.chunks.push({ name, value: line, inline });
      } else if (current.value.length + line.length + 1 > 1024) {
        acc.chunks.push({ name: `${name} (suite)`, value: line, inline });
      } else {
        current.value += `\n${line}`;
      }
      return acc;
    },
    { chunks: [] as EmbedField[] },
  );

  return result.chunks;
}

function statusFields(
  byStatus: Map<TransactionStatus, TransactionWithItems[]>,
  statuses: readonly TransactionStatus[],
  inline: boolean,
): EmbedField[] {
  return statuses.flatMap((status) => {
    const bucket = byStatus.get(status) ?? [];
    const name = `${TRANSACTION_STATUS_EMOJIS[status]} ${TRANSACTION_STATUS_LABELS[status]} (${bucket.length})`;
    return buildChunkedFields(name, bucket.map(formatLine), inline);
  });
}

/**
 * Tableau de bord des transactions, en un seul embed borné. Discord plafonne à
 * 6000 caractères pour l'ensemble des embeds d'un message : au-delà, l'édition
 * échoue et le tableau de bord se fige. `totalCount` compte toutes les
 * transactions de la guilde, y compris celles écartées de la liste.
 */
export function buildTransactionDashboardEmbed(
  transactions: TransactionWithItems[],
  totalCount: number,
): EmbedBuilder {
  const byStatus = new Map<TransactionStatus, TransactionWithItems[]>();
  for (const transaction of transactions) {
    const bucket = byStatus.get(transaction.statut) ?? [];
    bucket.push(transaction);
    byStatus.set(transaction.statut, bucket);
  }

  const { kept, dropped } = packFields(
    [
      ...statusFields(byStatus, ACTIVE_STATUSES, false),
      ...statusFields(byStatus, TERMINAL_STATUSES, true),
    ],
    { budget: FIELD_BUDGET, maxFields: MAX_FIELDS },
  );

  const activeCount = ACTIVE_STATUSES.reduce(
    (total, status) => total + (byStatus.get(status)?.length ?? 0),
    0,
  );
  const activeValue = ACTIVE_STATUSES.reduce(
    (total, status) =>
      total + (byStatus.get(status) ?? []).reduce((sum, item) => sum + item.prix, 0),
    0,
  );

  const embed = new EmbedBuilder()
    .setTitle("📊 Transactions")
    .setColor(0x5865f2)
    .addFields(kept);

  if (dropped.length > 0) {
    embed.addFields({
      name: "…",
      value: `${dropped.length} bloc(s) non affiché(s), faute de place. Utilise \`/transaction view\`.`,
    });
  }

  return embed
    .setFooter({
      text: `${activeCount} transaction(s) en cours · ${formatPrice(activeValue)} engagé(s) · ${totalCount} au total`,
    })
    .setTimestamp();
}
