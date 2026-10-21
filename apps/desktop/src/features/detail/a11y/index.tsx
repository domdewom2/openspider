import type { DetailPlugin } from "../frame/types";
import { Accessibility } from "./Accessibility";

export const detailTab: DetailPlugin = { id: "a11y", Component: Accessibility };
