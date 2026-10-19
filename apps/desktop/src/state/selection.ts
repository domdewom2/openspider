import type { Selection } from "../ipc/gen/Selection";

/** Immutable URL-key selection; selecting five million rows is constant space. */
export class SelectionModel {
  private readonly members: ReadonlySet<number>;
  private constructor(readonly value: Selection, readonly focus?: number, readonly anchor?: number) {
    this.members = new Set(value.kind === "all" ? value.except : value.ids);
  }
  static empty(): SelectionModel { return new SelectionModel({ kind: "ids", ids: [] }); }
  has(id: number): boolean { return this.value.kind === "all" ? !this.members.has(id) : this.members.has(id); }
  count(total: number): number { return this.value.kind === "all" ? Math.max(0, total - this.value.except.length) : this.value.ids.length; }
  select(id: number, toggle = false): SelectionModel {
    if (!toggle) return new SelectionModel({ kind: "ids", ids: [id] }, id, id);
    const key = this.value.kind === "all" ? "except" : "ids";
    const values = this.value.kind === "all" ? this.value.except : this.value.ids;
    const next = values.includes(id) ? values.filter(v => v !== id) : [...values, id];
    return new SelectionModel({ ...this.value, [key]: next }, id, id);
  }
  range(ids: number[], focus: number): SelectionModel { return new SelectionModel({ kind: "ids", ids: [...new Set(ids)] }, focus, this.anchor ?? focus); }
  all(viewId: string, version: number): SelectionModel { return new SelectionModel({ kind: "all", viewId, version, except: [] }, this.focus, this.anchor); }
  /** All-selection belongs to a particular snapshot, never silently expands after filtering. */
  forVersion(viewId: string, version: number): SelectionModel {
    return this.value.kind === "all" && (this.value.viewId !== viewId || this.value.version !== version) ? SelectionModel.empty() : this;
  }
}
