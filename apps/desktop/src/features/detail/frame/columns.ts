import { useEffect, useRef, useState } from "react";
import { scheduleDetailState } from "./persistence";
import { z } from "zod";
import { uistate } from "../../../ipc/uistate";
import type { ColumnState } from "../../../grid/DataGrid";
const columnSchema = z.array(z.object({ key: z.string(), width: z.number().min(32).max(4000), visible: z.boolean(), pinned: z.boolean(), sort: z.enum(["asc", "desc"]).optional() }));
/** Each detail table owns a layout, including separate layouts for Links and Anchors. */
export function useDetailColumns(id: string, onError: (error: unknown) => void): readonly [ColumnState[], (columns: ColumnState[]) => void] {
  const [state, setState] = useState<{ id: string; columns: ColumnState[]; edited: boolean }>({ id, columns: [], edited: false });
  const edited = useRef(false);
  useEffect(() => {
    let active = true; edited.current = false;
    void uistate.load().then(dto => {
      const root = dto.state && typeof dto.state === "object" && !Array.isArray(dto.state) ? dto.state : {};
      const layouts = root.detailColumns && typeof root.detailColumns === "object" && !Array.isArray(root.detailColumns) ? root.detailColumns : {};
      const result = columnSchema.safeParse(layouts[id]);
      if (active && !edited.current) setState({ id, columns: result.success ? result.data : [], edited: false });
    }).catch(onError);
    return () => { active = false; };
  }, [id]);
  return [state.id === id ? state.columns : [], columns => {
    edited.current = true; setState({ id, columns, edited: true });
    scheduleDetailState(`columns:${id}`, root => {
      const layouts = root.detailColumns && typeof root.detailColumns === "object" && !Array.isArray(root.detailColumns) ? root.detailColumns : {};
      return { ...root, detailColumns: { ...layouts, [id]: columns.map(column => ({ ...column })) } };
    }, onError);
  }];
}
