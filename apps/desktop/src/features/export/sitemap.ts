import { z } from "zod";

const frequency = z.enum(["always", "hourly", "daily", "weekly", "monthly", "yearly", "never"]);
export const sitemapSchema = z.object({
  include_noindex: z.boolean().default(false),
  include_canonicalised: z.boolean().default(false),
  include_paginated: z.boolean().default(false),
  include_pdfs: z.boolean().default(false),
  include_3xx: z.boolean().default(false),
  include_4xx: z.boolean().default(false),
  include_5xx: z.boolean().default(false),
  include_no_response: z.boolean().default(false),
  exclude_patterns: z.array(z.string()).default([]),
  lastmod: z.object({ enabled: z.boolean(), source: z.enum(["server", "custom"]), custom_date: z.string().nullable() }).default({ enabled: true, source: "server", custom_date: null }),
  priority: z.object({ enabled: z.boolean(), by_depth: z.array(z.number().min(0).max(1).multipleOf(0.1)).min(1) }).default({ enabled: true, by_depth: [1, 0.9, 0.8, 0.7, 0.6, 0.5] }),
  changefreq: z.object({ enabled: z.boolean(), basis: z.enum(["last_modified", "depth"]), by_depth: z.array(frequency).min(1) }).default({ enabled: true, basis: "last_modified", by_depth: ["daily", "daily", "weekly", "weekly", "monthly"] }),
  images: z.object({ enabled: z.boolean(), external_regex: z.array(z.string()), max_inlinks: z.number().int().nonnegative().nullable(), exclude_noindex: z.boolean() }).default({ enabled: false, external_regex: [], max_inlinks: null, exclude_noindex: false }),
  hreflang: z.object({ enabled: z.boolean(), only_valid_targets: z.boolean() }).default({ enabled: false, only_valid_targets: true }),
}).refine((s) => !s.lastmod.enabled || s.lastmod.source !== "custom" || validDate(s.lastmod.custom_date), { path: ["lastmod", "custom_date"], message: "export-date-invalid" });

function validDate(value: string | null): boolean {
  return !!value && /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(Date.parse(value)) && new Date(value).toISOString().slice(0, 10) === value;
}
export type SitemapOptions = z.infer<typeof sitemapSchema>;
export function defaultSitemapOptions(): SitemapOptions { return sitemapSchema.parse({}); }
