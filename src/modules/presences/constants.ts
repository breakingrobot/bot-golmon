import { ResponseStatus } from "@prisma/client";

export const RESPONSE_STATUS_LABELS: Record<ResponseStatus, string> = {
  [ResponseStatus.PRESENT]: "Présent",
  [ResponseStatus.ABSENT]: "Absent",
  [ResponseStatus.RETARD]: "Retard",
};

export const RESPONSE_STATUS_EMOJIS: Record<ResponseStatus, string> = {
  [ResponseStatus.PRESENT]: "✅",
  [ResponseStatus.ABSENT]: "❌",
  [ResponseStatus.RETARD]: "⏳",
};

export const RESPONSE_STATUS_CHOICES = Object.entries(RESPONSE_STATUS_LABELS).map(
  ([value, name]) => ({ name, value }),
);

export const PRESENCE_COLOR_ACTIVE = 0x5865f2;
export const PRESENCE_COLOR_CLOSED = 0x2c2f33;
