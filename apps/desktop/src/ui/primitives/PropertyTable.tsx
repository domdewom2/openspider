import type { ReactNode } from "react";

export interface PropertyRow {
  name: string;
  value: ReactNode;
}

/** Name/Value-Tabelle (URL Details, Eigenschaften). */
export function PropertyTable({ rows, caption, nameHeader, valueHeader }: { rows: readonly PropertyRow[]; caption: string; nameHeader: string; valueHeader: string }) {
  return (
    <table className="os-proptable">
      <caption className="os-visually-hidden">{caption}</caption>
      <thead>
        <tr>
          <th scope="col">{nameHeader}</th>
          <th scope="col">{valueHeader}</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.name}>
            <th scope="row">{r.name}</th>
            <td>{r.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
