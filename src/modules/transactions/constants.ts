import { TransactionStatus, TransactionType } from "@prisma/client";

export const TRANSACTION_STATUS_LABELS: Record<TransactionStatus, string> = {
  [TransactionStatus.PREPARATION]: "Préparation",
  [TransactionStatus.EN_ATTENTE_RDV]: "En attente RDV",
  [TransactionStatus.AVANCE_ATTENTE_PAIEMENT]: "Avance (en attente de paiement)",
  [TransactionStatus.LIVRE_PAYE]: "Livré et payé",
  [TransactionStatus.ANNULEE]: "Annulée",
};

export const TRANSACTION_STATUS_COLORS: Record<TransactionStatus, number> = {
  [TransactionStatus.PREPARATION]: 0x95a5a6,
  [TransactionStatus.EN_ATTENTE_RDV]: 0xf1c40f,
  [TransactionStatus.AVANCE_ATTENTE_PAIEMENT]: 0xe67e22,
  [TransactionStatus.LIVRE_PAYE]: 0x2ecc71,
  [TransactionStatus.ANNULEE]: 0xe74c3c,
};

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
