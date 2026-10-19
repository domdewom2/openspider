import type { ViewDataSource } from "../data/ViewDataSource";
import type { SelectionModel } from "../state/selection";

export async function copySelection(source: ViewDataSource, selection: SelectionModel, columns: string[], options: {
  confirmLarge: (rows: number) => Promise<boolean>;
  offerExport: () => void;
  withHeader?: boolean;
}): Promise<void> {
  const current = selection.forVersion(source.viewId, source.version);
  const rows = current.count(source.total);
  if (!rows || !columns.length) return;
  if (rows * columns.length > 1_000_000) { options.offerExport(); return; }
  if (rows > 100_000 && !await options.confirmLarge(rows)) return;
  // A confirmation dialog may span a view change; do not copy a different snapshot.
  if (current !== current.forVersion(source.viewId, source.version)) return;
  await source.api.copy({ viewId: source.viewId, selection: current.value, columns, withHeader: options.withHeader ?? true });
}
