import type { ReactNode } from "react";
import { Tab, TabList, TabPanel, Tabs as RacTabs, type Key } from "react-aria-components";
import { cx } from "./cx";

export interface TabSpec {
  id: string;
  label: ReactNode;
  /** Inhalt des Tab-Panels; nur das gewählte Panel wird gerendert. */
  content?: ReactNode;
}

/** Tab-Leiste mit Roving Tabindex (←/→, Pos1/Ende) – RAC liefert Tastatur und ARIA (`tablist`). */
export function Tabs({ tabs, selected, onSelect, label, orientation = "horizontal", className }: {
  tabs: readonly TabSpec[];
  selected: string;
  onSelect: (id: string) => void;
  label: string;
  orientation?: "horizontal" | "vertical";
  className?: string;
}) {
  return (
    <RacTabs
      selectedKey={selected}
      onSelectionChange={(k: Key) => onSelect(String(k))}
      orientation={orientation}
      className={cx("os-tabs", `os-tabs-${orientation}`, className)}
    >
      <TabList aria-label={label} className="os-tablist">
        {tabs.map((t) => (
          <Tab key={t.id} id={t.id} className="os-tab">
            {t.label}
          </Tab>
        ))}
      </TabList>
      {tabs.map((t) => (
        <TabPanel key={t.id} id={t.id} className="os-tabpanel">
          {t.content}
        </TabPanel>
      ))}
    </RacTabs>
  );
}
