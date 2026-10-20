import type { Bucket } from "../../../ipc/gen/Bucket";
import type { SearchSpec } from "../../../ipc/gen/SearchSpec";
import { andRules, cond, pct } from "../site-structure/model";

/** `0–1` … `9–10`, `10+` in seconds; the lower bound is inclusive, the upper one exclusive (UI-069). */
export function bucketLabel(b: Bucket): string {
  const from = b.fromMs / 1000;
  return b.toMs == null ? `${from}+` : `${from}–${b.toMs / 1000}`;
}

export interface BucketRow { bucket: Bucket; label: string; pct: number }

/** Shares refer to the URLs that did get a response; URLs without a response are reported separately. */
export function bucketRows(buckets: readonly Bucket[]): BucketRow[] {
  const sorted = [...buckets].sort((a, b) => a.fromMs - b.fromMs);
  const total = sorted.reduce((s, b) => s + b.count, 0);
  return sorted.map(bucket => ({ bucket, label: bucketLabel(bucket), pct: pct(bucket.count, total) }));
}

/** Seconds with the three decimals the table stores (`seconds3`), so `>= a` is expressed as `> a - 0.001`. */
const seconds = (ms: number): string => (ms / 1000).toFixed(3);

/** Click on a bucket: `Response Time ≥ a AND < b` (`≥ a` only for `10+`). */
export function bucketSearch(b: Bucket): SearchSpec {
  const children = [];
  if (b.fromMs > 0) children.push(cond("Response Time", "greaterThan", seconds(b.fromMs - 1)));
  if (b.toMs != null) children.push(cond("Response Time", "lessThan", seconds(b.toMs)));
  return andRules(...children);
}
