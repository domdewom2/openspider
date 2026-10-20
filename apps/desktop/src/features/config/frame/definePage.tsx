// Hilfsfunktion für Seitenmodule: Überschrift, optionaler Kopf- und Fußbereich und die Feldliste.
import type { ComponentType } from "react";
import { useT } from "../../../i18n";
import { FieldList, PageHeading } from "./fields";
import { pageTitleKey, type ConfigPage, type FieldDef } from "./types";

export function definePage(opts: {
  id: string;
  group?: string;
  order: number;
  fields: readonly FieldDef[];
  anchors?: readonly string[];
  searchKeys?: readonly string[];
  /** Fluent-Schlüssel eines Beschreibungstextes unter der Überschrift. */
  descriptionKey?: string;
  Before?: ComponentType;
  After?: ComponentType;
}): ConfigPage {
  const { Before, After } = opts;
  const titleKey = pageTitleKey(opts.id);
  function Component() {
    const t = useT();
    return (
      <section className="os-cfg-page" aria-label={t(titleKey)} data-page-id={opts.id}>
        <PageHeading titleKey={titleKey} descriptionKey={opts.descriptionKey} />
        {Before && <Before />}
        <FieldList fields={opts.fields} />
        {After && <After />}
      </section>
    );
  }
  return {
    id: opts.id,
    group: opts.group,
    order: opts.order,
    titleKey,
    anchors: opts.anchors ?? [],
    fields: opts.fields,
    searchKeys: opts.searchKeys,
    Component,
  };
}

/** Kurzform: ein Feld mit Standard-Beschriftung. */
export const f = (path: string, extra: Partial<FieldDef> = {}): FieldDef => ({ path, ...extra });
