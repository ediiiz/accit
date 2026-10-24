/** Nearest-rank percentile: the smallest sample with at least `fraction` of the samples at or below it. */
export function percentile(samples: number[], fraction: number): number {
  const sorted = samples.toSorted((a, b) => a - b);
  const rank = Math.max(1, Math.ceil(fraction * sorted.length));

  const value = sorted.at(rank - 1);

  if (value === undefined) throw new Error("percentile of no samples");

  return value;
}
