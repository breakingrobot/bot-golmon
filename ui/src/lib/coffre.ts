export const propertyLabel = (property: { id: string; name: string | null }): string =>
  property.name ?? `Propriété n° ${property.id}`
