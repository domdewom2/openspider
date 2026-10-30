// Zentraler keydown-Dispatcher mit Scopes (UI-113, M2-ui-concept 1.6): prüft `event.target`, filtert Kürzel nach Scope.
import { runAction } from "./actions";
import { canonicalCombo, combosOf, normalizeKey, SHORTCUTS, type ActionId, type Shortcut } from "./shortcuts";
import type { Platform } from "./tokens/tokens";

export interface KeyLike {
  key: string;
  code?: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export interface DispatchContext {
  platform: Platform;
  /** Ein Texteingabefeld hat den Fokus. */
  field: boolean;
  /** Der Fokus liegt in einer Tabelle (Region `table` bzw. `role="grid"`). */
  inTable: boolean;
  /** Ein modaler Dialog ist offen. */
  modal: boolean;
}

/** Kanonische Kombination eines Tastaturereignisses; Buchstaben und Ziffern über `code`, damit Alt/Option sie nicht verändert. */
export function eventCombo(e: KeyLike, platform: Platform): string {
  let key: string;
  if (e.code && /^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (e.code && /^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else key = normalizeKey(e.key);
  if (key === "Shift" || key === "Control" || key === "Alt" || key === "Meta") return "";
  // „+“ entsteht mit Umschalt: als Taste „+“ ohne zusätzliches Shift behandeln (Zoom).
  const shift = key === "+" ? false : e.shiftKey;
  const cmd = platform === "mac" ? e.metaKey : false;
  const ctrl = platform === "mac" ? e.ctrlKey : e.ctrlKey || false;
  return canonicalCombo(
    [ctrl && "Ctrl", e.altKey && "Alt", shift && "Shift", cmd && "Cmd", key].filter(Boolean).join("+"),
    platform,
  );
}

/** Welche Kürzel dürfen in einem Texteingabefeld global wirken (UI-113 Kontextregel). */
export function allowedInField(sc: Shortcut): boolean {
  return sc.scope === "field";
}

/** Reine Auflösung: welches Register-Kürzel gehört zum Ereignis in diesem Kontext? */
export function resolveShortcut(
  e: KeyLike,
  ctx: DispatchContext,
  list: readonly Shortcut[] = SHORTCUTS,
): Shortcut | null {
  // Modale Dialoge behandeln Esc und Enter selbst; globale Kürzel sind dort aus.
  if (ctx.modal) return null;
  const combo = eventCombo(e, ctx.platform);
  if (combo === "") return null;
  for (const sc of list) {
    if (!combosOf(sc, ctx.platform).includes(combo)) continue;
    if (ctx.field && !allowedInField(sc)) continue;
    if (sc.scope === "table" && !ctx.inTable) continue;
    return sc;
  }
  return null;
}

const TEXT_INPUT_TYPES = new Set(["text", "search", "url", "tel", "email", "password", "number", ""]);

export function isTextField(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  const el = target as HTMLElement;
  const ce = el.getAttribute("contenteditable");
  if (el.isContentEditable || (ce !== null && ce !== "false")) return true;
  const tag = el.tagName;
  if (tag === "TEXTAREA") return true;
  if (tag === "INPUT") return TEXT_INPUT_TYPES.has((el as HTMLInputElement).type);
  if (tag === "SELECT") return false;
  const role = el.getAttribute("role");
  return role === "textbox" || role === "searchbox" || role === "combobox";
}

export function isInTable(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[data-region="table"], [role="grid"]') !== null;
}

const MODAL_SELECTOR = '[aria-modal="true"], dialog[open], .os-modal, .os-popover';
const modalSnapshots = new WeakMap<Document, { observer: MutationObserver; dirty: boolean; open: boolean }>();

export function isModalOpen(doc: Document = document): boolean {
  // Menus check many actions against the same DOM. Invalidate on mutations,
  // including records not delivered yet so a same-turn action sees the new modal.
  const Observer = doc.defaultView?.MutationObserver;
  if (!Observer) return doc.querySelector(MODAL_SELECTOR) !== null;
  let snapshot = modalSnapshots.get(doc);
  if (!snapshot) {
    const entry = { observer: new Observer(() => { entry.dirty = true; }), dirty: true, open: false };
    entry.observer.observe(doc, { subtree: true, childList: true, attributes: true, attributeFilter: ["aria-modal", "open", "class"] });
    modalSnapshots.set(doc, entry);
    snapshot = entry;
  }
  if (snapshot.observer.takeRecords().length) snapshot.dirty = true;
  if (snapshot.dirty) {
    snapshot.open = doc.querySelector(MODAL_SELECTOR) !== null;
    snapshot.dirty = false;
  }
  return snapshot.open;
}

export interface DispatcherOptions {
  platform: Platform;
  root?: Document | HTMLElement;
  shortcuts?: readonly Shortcut[];
  /** Aktionen, die als native Menü-Accelerator laufen (das Betriebssystem fängt sie ab; kein Doppelauslösen). */
  nativeAccelerators?: () => ReadonlySet<ActionId>;
  /** Alternative zu `runAction`, z. B. in Tests. Rückgabe `false` = nicht behandelt. */
  onAction?: (id: ActionId, e: KeyboardEvent) => boolean | void;
}

/** Hängt den Dispatcher an `keydown`; liefert die Abmeldung. */
export function installShortcutDispatcher(opts: DispatcherOptions): () => void {
  const root = opts.root ?? document;
  const handler = (ev: Event) => {
    const e = ev as KeyboardEvent;
    if (e.defaultPrevented || e.isComposing) return;
    const sc = resolveShortcut(
      e,
      { platform: opts.platform, field: isTextField(e.target), inTable: isInTable(e.target), modal: isModalOpen() },
      opts.shortcuts ?? SHORTCUTS,
    );
    if (!sc) return;
    if (opts.nativeAccelerators?.().has(sc.id)) return;
    const handled = opts.onAction ? opts.onAction(sc.id, e) : runAction(sc.id, { source: "shortcut", event: e });
    if (handled !== false) {
      e.preventDefault();
      e.stopPropagation();
    }
  };
  root.addEventListener("keydown", handler, true);
  return () => root.removeEventListener("keydown", handler, true);
}
