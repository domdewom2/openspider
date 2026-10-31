export function median(values: number[]): number;
export function regression(
  value: number,
  previous: number[],
  direction?: "min" | "max",
): {
  status: string;
  baseline?: number;
  limit?: number;
};
export function report(directory?: string): {
  passed: boolean;
  [key: string]: unknown;
};
