import type { DetailPanelProps } from "../frame/types";
import { Links } from "../inlinks/Links";
export function Outlinks(props: DetailPanelProps) { return <Links {...props} direction="outlinks" />; }
