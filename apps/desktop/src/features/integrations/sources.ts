// Statischer Katalog der Integrationen bis Release 1.0 (INTEG-001, UI-070): feste Reihenfolge, Linkmetrik-Ebenen
// (INTEG-150) und Metriken je Linkmetrik-Anbieter. Verbindungsart und Zustand liefert ausschließlich `api.list`.
import type { ApiKind } from "../../ipc/gen/ApiKind";

/** Die sechs Einträge des Dialogs in fester Reihenfolge (UI-070); `drive` gehört zum Sheets-Export, nicht hierher. */
export const DIALOG_KINDS = ["ga4", "gsc", "psi", "majestic", "ahrefs", "moz"] as const satisfies readonly ApiKind[];
export type DialogKind = (typeof DIALOG_KINDS)[number];

export const LINK_METRIC_KINDS = ["majestic", "ahrefs", "moz"] as const satisfies readonly DialogKind[];
export type LinkMetricKind = (typeof LINK_METRIC_KINDS)[number];

export function isDialogKind(kind: string): kind is DialogKind {
  return (DIALOG_KINDS as readonly string[]).includes(kind);
}
export function isLinkMetricKind(kind: string): kind is LinkMetricKind {
  return (LINK_METRIC_KINDS as readonly string[]).includes(kind);
}

/** Anbietername in Spaltennamen (`<Anbieter> <Metrik> (<Ebene>)`, INTEG-150) – Schnittstellenbezeichner, nicht übersetzt. */
export const PROVIDER_NAME: Record<LinkMetricKind, string> = { majestic: "Majestic", ahrefs: "Ahrefs", moz: "Moz" };

/**
 * Ebenen aus INTEG-150. `url_both` (Exact URL, HTTP + HTTPS) kennt `ApiLevel` der Konfiguration noch nicht
 * (Schnittstellenbedarf, Bericht U26); die Ebene erscheint deshalb deaktiviert.
 */
export const LEVELS = ["url", "url_both", "subdomain", "domain"] as const;
export type Level = (typeof LEVELS)[number];
/** Ebenen, die `api.<quelle>.levels` speichern kann (`os_model::ApiLevel`). */
export const CONFIG_LEVELS: readonly Level[] = ["url", "subdomain", "domain"];

/** Kanonischer Ebenenname im Spaltennamen. */
export const LEVEL_NAME: Record<Level, string> = {
  url: "Exact URL",
  url_both: "Exact URL (HTTP + HTTPS)",
  subdomain: "Subdomain",
  domain: "Domain",
};

/** Vom Anbieter angebotene Ebenen (abgeleitet aus den Anbieter-APIs, Bericht U26). */
export const PROVIDER_LEVELS: Record<LinkMetricKind, readonly Level[]> = {
  majestic: ["url", "subdomain", "domain"],
  ahrefs: ["url", "url_both", "subdomain", "domain"],
  moz: ["url", "subdomain", "domain"],
};

export interface MetricDef {
  /** Gespeicherte Kennung in `api.<quelle>.metrics`. */
  id: string;
  /** Kanonischer englischer Name (Spaltenname). */
  name: string;
}

const m = (id: string, name: string): MetricDef => ({ id, name });

/** Metriken je Anbieter (INTEG-152 … 154); Kennungen folgen den Feldnamen der Anbieter-APIs. */
export const LINK_METRICS: Record<LinkMetricKind, readonly MetricDef[]> = {
  majestic: [
    m("ExtBackLinks", "External Backlinks"),
    m("RefDomains", "Referring Domains"),
    m("TrustFlow", "Trust Flow"),
    m("CitationFlow", "Citation Flow"),
    m("RefIPs", "Referring IPs"),
    m("RefSubNets", "Referring Subnets"),
    m("IndexedURLs", "Indexed URLs"),
    m("ExtBackLinksEDU", "External Backlinks EDU"),
    m("ExtBackLinksGOV", "External Backlinks GOV"),
    m("RefDomainsEDU", "Referring Domains EDU"),
    m("RefDomainsGOV", "Referring Domains GOV"),
  ],
  ahrefs: [
    m("domain_rating", "Domain Rating"),
    m("url_rating", "URL Rating"),
    m("backlinks", "Backlinks"),
    m("refdomains", "Referring Domains"),
    m("refpages", "Referring Pages"),
    m("backlinks_dofollow", "Dofollow Backlinks"),
    m("backlinks_nofollow", "Nofollow Backlinks"),
    m("backlinks_text", "Text Backlinks"),
    m("backlinks_image", "Image Backlinks"),
    m("backlinks_redirect", "Redirect Backlinks"),
    m("backlinks_canonical", "Canonical Backlinks"),
    m("backlinks_gov", "Gov Backlinks"),
    m("backlinks_edu", "Edu Backlinks"),
    m("refdomains_gov", "Gov Referring Domains"),
    m("refdomains_edu", "Edu Referring Domains"),
    m("refips", "Referring IPs"),
    m("refclass_c", "Referring Class C"),
    m("linked_root_domains", "Linked Root Domains"),
    m("outlinks_internal", "Outgoing Links Internal"),
    m("outlinks_external", "Outgoing Links External"),
    m("org_keywords", "Organic Keywords"),
    m("org_keywords_top3", "Organic Keywords Top 3"),
    m("org_keywords_top10", "Organic Keywords Top 10"),
    m("org_traffic", "Organic Traffic"),
    m("org_traffic_top3", "Organic Traffic Top 3"),
    m("org_traffic_top10", "Organic Traffic Top 10"),
    m("traffic_value", "Traffic Value"),
  ],
  moz: [
    m("page_authority", "Page Authority"),
    m("domain_authority", "Domain Authority"),
    m("spam_score", "Spam Score"),
    m("brand_authority", "Brand Authority"),
    m("link_propensity", "Link Propensity"),
    m("root_domains_to_page", "Root Domains Linking"),
    m("external_pages_to_page", "External Pages Linking"),
    m("pages_to_page", "Pages Linking"),
    m("nofollow_root_domains_to_page", "Nofollow Root Domains"),
    m("last_crawled", "Last Crawled"),
  ],
};

/** Spaltennamen `<Anbieter> <Metrik> (<Ebene>)` in Metrik-, dann Ebenenreihenfolge (INTEG-150). */
export function linkMetricColumns(kind: LinkMetricKind, metrics: readonly string[], levels: readonly string[]): string[] {
  const offered = PROVIDER_LEVELS[kind].filter((l) => CONFIG_LEVELS.includes(l) && levels.includes(l));
  const out: string[] = [];
  for (const def of LINK_METRICS[kind]) {
    if (!metrics.includes(def.id)) continue;
    for (const level of offered) out.push(`${PROVIDER_NAME[kind]} ${def.name} (${LEVEL_NAME[level]})`);
  }
  return out;
}

/** Eine Ebene ist im Dialog bedienbar, wenn der Anbieter sie anbietet und die Konfiguration sie speichern kann. */
export function levelSelectable(kind: LinkMetricKind, level: Level): boolean {
  return PROVIDER_LEVELS[kind].includes(level) && CONFIG_LEVELS.includes(level);
}
