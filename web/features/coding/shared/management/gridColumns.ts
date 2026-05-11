export const MANAGEMENT_GRID_COLUMN_OPTIONS = ['auto', 1, 2, 3, 4, 5] as const;

export type ManagementGridColumnSetting = typeof MANAGEMENT_GRID_COLUMN_OPTIONS[number];

export function parseManagementGridColumnSetting(value: string): ManagementGridColumnSetting {
  return value === 'auto' ? 'auto' : Number(value) as ManagementGridColumnSetting;
}
