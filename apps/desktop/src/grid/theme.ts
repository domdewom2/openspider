import { SIZES } from "../ui/tokens/tokens";

/** K2-77: same physical head heights as the CSS/data-view density. */
export const gridHeaderHeight = (rowHeight: number) => rowHeight === SIZES.rowHeight.normal ? SIZES.headerHeight.normal : rowHeight + SIZES.headerHeight.compact - SIZES.rowHeight.compact;
