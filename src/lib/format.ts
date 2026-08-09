/**
 * Limites Discord : 1024 caractères par champ d'embed, 2000 par message.
 * Une mention fait ~22 caractères, donc un rôle d'une cinquantaine de membres
 * suffit à dépasser la limite d'un champ et à faire échouer l'edit
 * (`Invalid Form Body`). Ces helpers bornent systématiquement les listes.
 */
const FIELD_LIMIT = 1024;
const MESSAGE_LIMIT = 2000;

export function mention(userId: string): string {
  return `<@${userId}>`;
}

/**
 * Liste de mentions pour un champ d'embed, tronquée à `max` entrées puis à la
 * limite de caractères. Renvoie "Aucun" si la liste est vide.
 */
export function formatMentionList(
  userIds: string[],
  options: { max?: number; detailByUserId?: Map<string, string>; empty?: string } = {},
): string {
  const { max = Infinity, detailByUserId, empty = "Aucun" } = options;

  if (userIds.length === 0) {
    return empty;
  }

  const shown = userIds.slice(0, max);
  const lines = shown.map((id) => {
    const detail = detailByUserId?.get(id);
    return detail ? `${mention(id)} (${detail})` : mention(id);
  });

  let remaining = userIds.length - shown.length;
  let text = lines.join("\n");

  // Retire des entrées tant qu'on dépasse la limite du champ.
  while (text.length > FIELD_LIMIT - 20 && lines.length > 0) {
    lines.pop();
    remaining += 1;
    text = lines.join("\n");
  }

  return remaining > 0 ? `${text}\n… +${remaining} autre${remaining > 1 ? "s" : ""}` : text;
}

/**
 * Découpe une liste de mentions en plusieurs messages sous la limite de 2000
 * caractères, chacun préfixé par `prefix`.
 */
export function chunkMentions(userIds: string[], prefix: string): string[] {
  const chunks: string[] = [];
  let current = "";

  for (const userId of userIds) {
    const candidate = current ? `${current} ${mention(userId)}` : `${prefix} ${mention(userId)}`;
    if (candidate.length > MESSAGE_LIMIT) {
      chunks.push(current);
      current = `${prefix} ${mention(userId)}`;
    } else {
      current = candidate;
    }
  }

  if (current) {
    chunks.push(current);
  }

  return chunks;
}

/** Divise une liste en morceaux de taille maximale `chunkSize`. */
export function chunkMentionsList(userIds: string[], chunkSize: number): string[][] {
  const result: string[][] = [];
  for (let i = 0; i < userIds.length; i += chunkSize) {
    result.push(userIds.slice(i, i + chunkSize));
  }
  return result;
}

/** Tronque un texte libre pour un champ d'embed. */
export function truncateField(text: string, limit = FIELD_LIMIT): string {
  return text.length <= limit ? text : `${text.slice(0, limit - 1)}…`;
}
