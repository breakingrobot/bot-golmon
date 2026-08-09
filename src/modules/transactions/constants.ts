import { TransactionStatus, TransactionType } from "@prisma/client";

export const TRANSACTION_STATUS_LABELS: Record<TransactionStatus, string> = {
  [TransactionStatus.PREPARATION]: "Préparation",
  [TransactionStatus.EN_ATTENTE_RDV]: "En attente RDV",
  [TransactionStatus.AVANCE_ATTENTE_PAIEMENT]: "Avance (en attente de paiement)",
  [TransactionStatus.LIVRE_PAYE]: "Livré et payé",
  [TransactionStatus.ANNULEE]: "Annulée",
};

export const TRANSACTION_STATUS_EMOJIS: Record<TransactionStatus, string> = {
  [TransactionStatus.PREPARATION]: "🔧",
  [TransactionStatus.EN_ATTENTE_RDV]: "📅",
  [TransactionStatus.AVANCE_ATTENTE_PAIEMENT]: "💰",
  [TransactionStatus.LIVRE_PAYE]: "✅",
  [TransactionStatus.ANNULEE]: "❌",
};

export const TRANSACTION_STATUS_COLORS: Record<TransactionStatus, number> = {
  [TransactionStatus.PREPARATION]: 0x95a5a6,
  [TransactionStatus.EN_ATTENTE_RDV]: 0xf1c40f,
  [TransactionStatus.AVANCE_ATTENTE_PAIEMENT]: 0xe67e22,
  [TransactionStatus.LIVRE_PAYE]: 0x2ecc71,
  [TransactionStatus.ANNULEE]: 0xe74c3c,
};

/** Statuts en cours, listés intégralement dans le tableau de bord. */
export const ACTIVE_STATUSES = [
  TransactionStatus.PREPARATION,
  TransactionStatus.EN_ATTENTE_RDV,
  TransactionStatus.AVANCE_ATTENTE_PAIEMENT,
] as const;

/** Statuts terminaux : seuls les plus récents sont affichés, pour borner la taille. */
export const TERMINAL_STATUSES = [
  TransactionStatus.LIVRE_PAYE,
  TransactionStatus.ANNULEE,
] as const;

export const TRANSACTION_TYPE_LABELS: Record<TransactionType, string> = {
  [TransactionType.ACHAT]: "Achat",
  [TransactionType.VENTE]: "Vente",
};

export const TRANSACTION_STATUS_CHOICES = Object.entries(TRANSACTION_STATUS_LABELS).map(
  ([value, name]) => ({ name, value }),
);

export const TRANSACTION_TYPE_CHOICES = Object.entries(TRANSACTION_TYPE_LABELS).map(
  ([value, name]) => ({ name, value }),
);

export function formatPrice(value: number): string {
  return new Intl.NumberFormat("fr-FR").format(value);
}
