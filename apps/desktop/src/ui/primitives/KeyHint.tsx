import { useLanguageStore } from "../../i18n";
import { currentPlatform, type Platform } from "../platform";
import { formatShortcut, shortcutById, type ActionId } from "../shortcuts";

/** Kürzelanzeige in Plattformschreibweise, aus dem Register (UI-113): `⌘⇧A` bzw. `Strg+Umschalt+A`. */
export function KeyHint({ action, combo, platform = currentPlatform() }: { action?: ActionId; combo?: string; platform?: Platform }) {
  const lang = useLanguageStore((s) => s.lang);
  const raw = combo ?? (action ? shortcutById(action)?.keys[platform] : undefined);
  if (!raw) return null;
  return <kbd className="os-keyhint">{formatShortcut(raw, platform, lang)}</kbd>;
}
