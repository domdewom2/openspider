import { registerActionGuard } from "../../ui/actions";
import { allowedInField, isInTable, isModalOpen, isTextField } from "../../ui/dispatcher";
import { shortcutById } from "../../ui/shortcuts";

/** Apply the dispatcher scopes to native callbacks too, including callbacks queued before a focus change. */
export function menuContextReason(id: string): string | undefined {
  const shortcut = shortcutById(id);
  if (!shortcut || typeof document === "undefined") return undefined;
  if (isModalOpen()) return "u04-context-modal";
  if (isTextField(document.activeElement) && !allowedInField(shortcut)) return "u04-context-field";
  return shortcut.scope === "table" && !isInTable(document.activeElement) ? "u04-context-table" : undefined;
}

export const menuContextAllows = (id: string): boolean => menuContextReason(id) === undefined;

export function installMenuContexts(rebuild: () => void): () => void {
  const off = registerActionGuard((id, ctx) => ctx.source !== "menu" || menuContextAllows(id));
  const context = () => [isModalOpen(), isTextField(document.activeElement), isInTable(document.activeElement)].join();
  let previous = context();
  const changed = () => {
    const next = context();
    if (next === previous) return;
    previous = next;
    rebuild();
  };
  const observer = new MutationObserver(changed);
  observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["aria-modal", "open", "role", "contenteditable", "type", "data-region"] });
  document.addEventListener("focusin", changed);
  document.addEventListener("focusout", changed);
  return () => {
    off();
    observer.disconnect();
    document.removeEventListener("focusin", changed);
    document.removeEventListener("focusout", changed);
  };
}
