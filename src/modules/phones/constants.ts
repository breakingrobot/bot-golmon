export const PHONE_NAMESPACE = "numero";

/** Format RP attendu : 555 suivi de 4 à 6 chiffres (555XXXX à 555XXXXXX). */
export const PHONE_REGEX = /^555\d{4,6}$/;

export const INVALID_FORMAT =
  "Format invalide. Un numéro commence par 555 suivi de 4 à 6 chiffres, par exemple `5551234`.";

export function normalizePhone(input: string): string {
  return input.replace(/\D/g, "");
}

export function formatPhone(numero: string): string {
  return numero.replace(/^555/, "555-");
}
