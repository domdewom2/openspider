// Fensterlabel der Visualisierungen ohne IPC-Abhängigkeiten (auch in Node-Testhilfen nutzbar).

/** Fensterlabel wie im Kern: `viz-` + Hex der UTF-8-Bytes der Schnappschuss-Kennung. */
export function windowLabelFor(snapshotId: string): string {
  return `viz-${[...new TextEncoder().encode(snapshotId)].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
}
