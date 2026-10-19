import { useT } from "../../../../i18n";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";

export function McpReference() {
  const t = useT();
  return <p data-setting-anchor="mcp.server" tabIndex={-1}>{t("u14-mcp-reference")}</p>;
}
export const section: SettingsSection = { id: "mcp", order: 90, titleKey: sectionTitleKey("mcp"), Component: McpReference };
