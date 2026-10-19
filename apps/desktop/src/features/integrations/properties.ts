// Auswahl von Konto, Property und Datenstream (GA4, INTEG-090) bzw. Search-Console-Property (INTEG-110).
// Die Listen kommen von einem Lader; der Command-Vertrag (M2.md 3.6a) kennt noch keine Property-Listen, deshalb ist der
// Lader ein Einhängepunkt (Schnittstellenbedarf im Bericht U26). Ohne Lader bleibt die manuelle Eingabe der Kennung.
import { create } from "zustand";

export interface Ga4Stream {
  id: string;
  name: string;
  /** Hostname des Web-Datenstreams; schränkt die Daten ein (Filter auf `hostName`). */
  hostname: string | null;
}
export interface Ga4Property {
  id: string;
  name: string;
  streams: readonly Ga4Stream[];
}
export interface Ga4Account {
  id: string;
  name: string;
  properties: readonly Ga4Property[];
}
export interface GscSite {
  /** `sc-domain:example.com` oder URL-Präfix `https://example.com/`. */
  siteUrl: string;
  /** Berechtigungsstufe der Search Console, z. B. `siteOwner`, `siteUnverifiedUser`. */
  permissionLevel: string;
}

export interface PropertyCatalog {
  ga4?: (googleAccountId: string | null, signal: AbortSignal) => Promise<readonly Ga4Account[]>;
  gsc?: (googleAccountId: string | null, signal: AbortSignal) => Promise<readonly GscSite[]>;
}

export const usePropertyCatalog = create<{ catalog: PropertyCatalog }>(() => ({ catalog: {} }));

/** Meldet einen Lader für Property-Listen an (Kern-Command, sobald vorhanden); liefert die Abmeldung. */
export function registerPropertyCatalog(catalog: PropertyCatalog): () => void {
  usePropertyCatalog.setState({ catalog });
  return () => {
    if (usePropertyCatalog.getState().catalog === catalog) usePropertyCatalog.setState({ catalog: {} });
  };
}

// --- GA4 (INTEG-090) -----------------------------------------------------------------------------------------------

/** Properties nur des gewählten Kontos (abhängige Auswahl), optional nach Suchtext gefiltert. */
export function propertiesOf(accounts: readonly Ga4Account[], accountId: string | null, search = ""): readonly Ga4Property[] {
  const account = accounts.find((a) => a.id === accountId);
  if (!account) return [];
  const q = search.trim().toLowerCase();
  if (!q) return account.properties;
  return account.properties.filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q));
}

export function streamsOf(accounts: readonly Ga4Account[], propertyId: string | null): readonly Ga4Stream[] {
  for (const a of accounts) for (const p of a.properties) if (p.id === propertyId) return p.streams;
  return [];
}

/** Konto, zu dem eine gespeicherte Property gehört (Vorauswahl bei erneuter Verbindung). */
export function accountOfProperty(accounts: readonly Ga4Account[], propertyId: string | null | undefined): string | null {
  if (!propertyId) return accounts[0]?.id ?? null;
  return accounts.find((a) => a.properties.some((p) => p.id === propertyId))?.id ?? accounts[0]?.id ?? null;
}

/** Hostnamen-Filter der Abfrage; „Alle Datenstreams“ (`null`) filtert nicht. */
export function streamHostFilter(stream: Ga4Stream | null | undefined): string | null {
  return stream?.hostname ? `hostName==${stream.hostname}` : null;
}

/** Anzeigeteile für `Verbunden (<Konto> / <Property> / <Stream>)`. */
export function ga4StatusParts(
  account: string | null | undefined,
  accounts: readonly Ga4Account[],
  propertyId: string | null | undefined,
  streamId: string | null | undefined,
): string[] {
  const parts: string[] = [];
  if (account) parts.push(account);
  for (const a of accounts) {
    const p = a.properties.find((x) => x.id === propertyId);
    if (!p) continue;
    parts.push(a.name, p.name);
    const s = p.streams.find((x) => x.id === streamId);
    if (s) parts.push(s.name);
    return parts;
  }
  if (propertyId) parts.push(propertyId);
  return parts;
}

// --- Search Console (INTEG-110) ------------------------------------------------------------------------------------

export type GscSiteKind = "domain" | "prefix";

export function gscSiteKind(siteUrl: string): GscSiteKind {
  return siteUrl.startsWith("sc-domain:") ? "domain" : "prefix";
}

/** Unbestätigte Properties sind ausgegraut. */
export function gscSiteUnverified(site: GscSite): boolean {
  return site.permissionLevel === "siteUnverifiedUser";
}

function parse(url: string): URL | null {
  try {
    return new URL(url);
  } catch {
    return null;
  }
}

/**
 * Passt die Property zur Crawl-Domain? Domain-Property: Host gleich oder Subdomain; URL-Präfix: die Crawl-URL beginnt
 * mit dem Präfix (Schema und Host ohne Beachtung der Groß-/Kleinschreibung). Ohne Crawl-URL gibt es keinen Hinweis.
 */
export function gscSiteMatchesCrawl(siteUrl: string, crawlUrl: string | null | undefined): boolean {
  if (!crawlUrl) return true;
  const crawl = parse(crawlUrl);
  if (!crawl) return true;
  const host = crawl.hostname.toLowerCase();
  if (gscSiteKind(siteUrl) === "domain") {
    const domain = siteUrl.slice("sc-domain:".length).toLowerCase().replace(/\.$/, "");
    return host === domain || host.endsWith(`.${domain}`);
  }
  const prefix = parse(siteUrl);
  if (!prefix) return false;
  if (prefix.protocol !== crawl.protocol || prefix.host.toLowerCase() !== crawl.host.toLowerCase()) return false;
  const path = crawl.pathname === "" ? "/" : crawl.pathname;
  return path.startsWith(prefix.pathname || "/");
}

/** Domain-Properties zuerst, innerhalb alphabetisch. */
export function sortGscSites(sites: readonly GscSite[]): GscSite[] {
  return [...sites].sort((a, b) => {
    const k = (s: GscSite) => (gscSiteKind(s.siteUrl) === "domain" ? 0 : 1);
    return k(a) - k(b) || a.siteUrl.localeCompare(b.siteUrl);
  });
}

export function gscSiteName(siteUrl: string): string {
  return gscSiteKind(siteUrl) === "domain" ? siteUrl.slice("sc-domain:".length) : siteUrl;
}
