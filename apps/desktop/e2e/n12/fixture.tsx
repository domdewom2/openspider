import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { setLanguage } from "../../src/i18n";
import { applyAppearance, DEFAULT_APPEARANCE } from "../../src/ui/tokens/theme";
import { ComboBox } from "../../src/ui/primitives/ComboBox";
import { IssueBadge } from "../../src/features/sidebar/issues/IssueBadge";
import { CodePane, type CodePaneHandle } from "../../src/features/detail/source/CodePane";
import "../../src/ui/ui.css";
import "../../src/features/sidebar/frame/sidebar.css";
import "../../src/features/detail/source/source.css";

function Fixture() {
  const [selected, setSelected] = useState("one");
  const pane = useRef<CodePaneHandle>(null);
  useEffect(() => {
    pane.current?.append("added line\nremoved line");
    pane.current?.setOverlay({ lineKinds: new Map([[1, "added"], [2, "removed"]]) });
  }, []);
  return <main>
    <ComboBox label="Choices" selected={selected} onSelect={setSelected} options={[{ id: "one", label: "One" }, { id: "two", label: "Two" }]} />
    <output data-testid="choice">{selected}</output>
    <div className="os-sidebar">{(["issue", "warning", "opportunity"] as const).map(issueType =>
      <IssueBadge key={issueType} badge={{ issueType, priority: "medium" }} />)}</div>
    <CodePane ref={pane} label="Diff fixture" wrap={false} highlight={false} />
  </main>;
}
await setLanguage("en");
applyAppearance(document.documentElement, { ...DEFAULT_APPEARANCE, theme: "light" }, { platform: "win", systemDark: false });
createRoot(document.getElementById("root")!).render(<Fixture />);
