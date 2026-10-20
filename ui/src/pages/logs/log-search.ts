export type LogSearch = { simple?: 1; verbose?: 1 }

export const flag = (value: unknown): 1 | undefined => (value === 1 || value === '1' || value === true ? 1 : undefined)
