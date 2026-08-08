const SEPARATOR = ":";

export function encodeCustomId(namespace: string, action: string, id: string): string {
  return [namespace, action, id].join(SEPARATOR);
}

export function decodeCustomId(customId: string): { namespace: string; action: string; id: string } {
  const [namespace, action, id] = customId.split(SEPARATOR);
  if (!namespace || !action || !id) {
    throw new Error(`customId invalide: ${customId}`);
  }
  return { namespace, action, id };
}
