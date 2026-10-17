import { Button, Tree as RacTree, TreeItem, TreeItemContent } from "react-aria-components";
import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

export interface TreeNode {
  id: string;
  label: ReactNode;
  textValue: string;
  children?: readonly TreeNode[];
}

function renderNode(n: TreeNode): ReactNode {
  return (
    <TreeItem key={n.id} id={n.id} textValue={n.textValue} className="os-tree-item">
      <TreeItemContent>
        {({ hasChildItems }) => (
          <>
            {hasChildItems ? (
              <Button slot="chevron" className="os-tree-toggle">
                <ChevronRight size={12} aria-hidden className="os-tree-chevron" />
              </Button>
            ) : (
              <span className="os-tree-chevron" />
            )}
            <span>{n.label}</span>
          </>
        )}
      </TreeItemContent>
      {n.children?.map(renderNode)}
    </TreeItem>
  );
}

/** Baum (Overview, Site Structure, Projektbaum): Pfeiltasten, Auf-/Zuklappen und ARIA liefert RAC. */
export function Tree({ nodes, label, selected, onSelect }: { nodes: readonly TreeNode[]; label: string; selected?: string | null; onSelect?: (id: string) => void }) {
  return (
    <RacTree
      aria-label={label}
      className="os-tree"
      selectionMode={onSelect ? "single" : "none"}
      selectedKeys={selected ? [selected] : []}
      onSelectionChange={(keys) => {
        const first = [...keys][0];
        if (first !== undefined && onSelect) onSelect(String(first));
      }}
    >
      {nodes.map(renderNode)}
    </RacTree>
  );
}
