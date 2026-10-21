import type { DetailPlugin } from "../frame/types";
import { ViewSource } from "./ViewSource";

export const detailTab: DetailPlugin = { id: "source", Component: ViewSource };
