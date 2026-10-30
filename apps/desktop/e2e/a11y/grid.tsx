// Functional virtualisation fixture; deliberately no timings or performance assertions.
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { DataGrid, defaultColumns } from "../../src/grid/DataGrid";
import { ViewDataSource } from "../../src/data/ViewDataSource";
import { SelectionModel } from "../../src/state/selection";
import { resolveTokens } from "../../src/ui/tokens/tokens";
import fixture from "../../src/grid/fixtures/fx-ui-basic.json";
import type { ViewHandle } from "../../src/ipc/gen/ViewHandle";
import "../../src/ui/ui.css";

const source = new ViewDataSource({ ...fixture, total: 5000 } as ViewHandle, {
  rows: async args => ({ ...args, rowKeys: Array.from({ length: args.limit }, (_, i) => args.offset + i + 1),
    cells: Array.from({ length: args.limit }, (_, i) => [args.offset + i + 1, ...fixture.row.slice(1)]) }),
  locate: async args => args.urlId - 1, copy: async () => ({ rows: 1, bytes: 1 }), close: async () => {},
});
function Fixture() {
  const [selection, setSelection] = useState(SelectionModel.empty());
  const [columns, setColumns] = useState(defaultColumns(source.columns));
  return <main><h1>Virtual grid fixture</h1><button>Before grid</button><div style={{ height: 300 }}>
    <DataGrid source={source} columns={columns} selection={selection} onSelectionChange={setSelection} onColumnsChange={setColumns}
      onSortChange={() => {}} onContextMenu={() => {}} onActivate={() => {}} onError={error => { throw error; }}
      confirmLargeCopy={async () => false} onExportSelection={() => {}} rowHeight={28}
      theme={{ tokens: resolveTokens("light", "#2f855a"), fontFamily: "system-ui", fontSize: 12, zebra: true }} ariaLabel="Virtual fixture" locale="en" />
  </div><button>After grid</button></main>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
