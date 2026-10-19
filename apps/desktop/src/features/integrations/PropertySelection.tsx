// Auswahl nach der Google-Anmeldung: GA4 Konto → Property → Datenstream (INTEG-090), Search Console Property (INTEG-110).
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { toCmdError } from "../../ipc/errors";
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import { Banner, ComboBox, SearchField, Select, TextField } from "../../ui/primitives";
import {
  accountOfProperty,
  gscSiteKind,
  gscSiteMatchesCrawl,
  gscSiteName,
  gscSiteUnverified,
  propertiesOf,
  sortGscSites,
  streamsOf,
  type Ga4Account,
  type GscSite,
} from "./properties";
import { useIntegrations } from "./store";
import { str, type Values } from "./settings";

export type CatalogState<T> = { status: "unavailable" | "loading" | "error"; items: readonly T[] } | { status: "ready"; items: readonly T[] };

/** Lädt die Property-Liste über den angemeldeten Lader, sobald die Quelle verbunden ist. */
export function useCatalogList<T>(loader: ((account: string | null, signal: AbortSignal) => Promise<readonly T[]>) | undefined, enabled: boolean, account: string | null): CatalogState<T> {
  const [state, setState] = useState<CatalogState<T>>({ status: loader ? "loading" : "unavailable", items: [] });
  useEffect(() => {
    if (!loader) {
      setState({ status: "unavailable", items: [] });
      return;
    }
    if (!enabled) {
      setState({ status: "loading", items: [] });
      return;
    }
    const ctrl = new AbortController();
    setState({ status: "loading", items: [] });
    loader(account, ctrl.signal).then(
      (items) => !ctrl.signal.aborted && setState({ status: "ready", items }),
      (e) => !ctrl.signal.aborted && toCmdError(e).code !== "Cancelled" && setState({ status: "error", items: [] }),
    );
    return () => ctrl.abort();
  }, [loader, enabled, account]);
  return state;
}

type Change = (path: readonly string[], value: JsonValue) => void;

const ALL_STREAMS = "__all__";

export function Ga4Selection({ values, onChange, catalog }: { values: Values; onChange: Change; catalog: CatalogState<Ga4Account> }) {
  const t = useT();
  const property = str(values.property);
  const [account, setAccount] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const accounts = catalog.items;
  useEffect(() => {
    if (catalog.status === "ready") setAccount((cur) => (cur && accounts.some((a) => a.id === cur) ? cur : accountOfProperty(accounts, property)));
  }, [catalog.status, accounts, property]);

  if (catalog.status === "unavailable" || catalog.status === "error") {
    return (
      <div className="os-integ-selection">
        <TextField label={t("integ-ga4-property-id")} value={property ?? ""} onChange={(v) => onChange(["property"], v.trim() === "" ? null : v.trim())} description={t(catalog.status === "error" ? "integ-catalog-error" : "integ-catalog-unavailable")} />
      </div>
    );
  }
  if (catalog.status === "loading") return <p className="os-hint" role="status">{t("integ-catalog-loading")}</p>;

  const properties = propertiesOf(accounts, account);
  const visible = propertiesOf(accounts, account, search);
  const streams = streamsOf(accounts, property);
  const stream = str(values.stream);
  const accountName = accounts.find((a) => a.id === account)?.name ?? "";
  return (
    <div className="os-integ-selection">
      <Select
        label={t("integ-ga4-account")}
        value={account ?? ""}
        options={accounts.map((a) => ({ value: a.id, label: a.name }))}
        onChange={(id) => {
          setAccount(id);
          setSearch("");
          if (!propertiesOf(accounts, id).some((p) => p.id === property)) {
            onChange(["property"], null);
            onChange(["stream"], null);
          }
        }}
      />
      {properties.length === 0 ? (
        <Banner kind="info">{t("integ-ga4-no-properties", { account: accountName })}</Banner>
      ) : (
        <>
          {properties.length > 1 && <SearchField label={t("integ-ga4-property-search")} value={search} onChange={setSearch} clearLabel={t("ui-clear-search")} />}
          <Select
            label={t("integ-ga4-property")}
            value={property ?? ""}
            options={visible.map((p) => ({ value: p.id, label: `${p.name} (${p.id})` }))}
            onChange={(id) => {
              onChange(["property"], id);
              onChange(["stream"], null);
            }}
          />
          {property && (
            <Select
              label={t("integ-ga4-stream")}
              value={stream ?? ALL_STREAMS}
              options={[{ value: ALL_STREAMS, label: t("integ-ga4-all-streams") }, ...streams.map((s) => ({ value: s.id, label: s.hostname ? `${s.name} (${s.hostname})` : s.name }))]}
              onChange={(id) => onChange(["stream"], id === ALL_STREAMS ? null : id)}
            />
          )}
        </>
      )}
    </div>
  );
}

export function GscSelection({ values, onChange, catalog }: { values: Values; onChange: Change; catalog: CatalogState<GscSite> }) {
  const t = useT();
  const crawlUrl = useIntegrations((s) => s.crawlUrl);
  const site = str(values.site);
  const mismatch = site !== null && !gscSiteMatchesCrawl(site, crawlUrl);
  const warning = mismatch && <Banner kind="warning">{t("integ-gsc-mismatch")}</Banner>;
  if (catalog.status === "unavailable" || catalog.status === "error") {
    return (
      <div className="os-integ-selection">
        <TextField label={t("integ-gsc-property-id")} value={site ?? ""} onChange={(v) => onChange(["site"], v.trim() === "" ? null : v.trim())} description={t(catalog.status === "error" ? "integ-catalog-error" : "integ-catalog-unavailable")} />
        {warning}
      </div>
    );
  }
  if (catalog.status === "loading") return <p className="os-hint" role="status">{t("integ-catalog-loading")}</p>;
  const sites = sortGscSites(catalog.items);
  if (sites.length === 0) return <Banner kind="info">{t("integ-gsc-no-properties")}</Banner>;
  return (
    <div className="os-integ-selection">
      <ComboBox
        label={t("integ-gsc-property")}
        selected={site}
        onSelect={(id) => onChange(["site"], id)}
        options={sites.map((s) => {
          const unverified = gscSiteUnverified(s);
          const kind = t(gscSiteKind(s.siteUrl) === "domain" ? "integ-gsc-kind-domain" : "integ-gsc-kind-prefix");
          return {
            id: s.siteUrl,
            label: `${gscSiteName(s.siteUrl)} – ${kind}${unverified ? ` (${t("integ-gsc-unverified")})` : ""}`,
            isDisabled: unverified,
            hint: unverified ? t("integ-gsc-unverified") : undefined,
          };
        })}
      />
      {warning}
    </div>
  );
}
