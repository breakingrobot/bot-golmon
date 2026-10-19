export function propertyLabel(property: { id: string; name: string | null }): string {
  return property.name ?? `Propriété n° ${property.id}`
}
