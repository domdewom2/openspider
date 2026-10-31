import "./contrast.css";
import type { ReactNode } from "react";
import { Button as RacButton, type ButtonProps as RacButtonProps } from "react-aria-components";
import { cx } from "./cx";

export type ButtonVariant = "primary" | "secondary" | "tool" | "danger";

export interface ButtonProps extends Omit<RacButtonProps, "className"> {
  variant?: ButtonVariant;
  className?: string;
}

/** Schaltfläche (primär/sekundär/Werkzeug); Höhe 28 px in Dialogen, 24 px in Werkzeugzeilen. */
export function Button({ variant = "secondary", className, ...rest }: ButtonProps) {
  return <RacButton {...rest} className={cx("os-btn", `os-btn-${variant}`, className)} />;
}

export interface IconButtonProps extends Omit<RacButtonProps, "className" | "children"> {
  /** Pflicht: zugänglicher Name (Tooltip-Text). */
  label: string;
  icon: ReactNode;
  className?: string;
}

export function IconButton({ label, icon, className, ...rest }: IconButtonProps) {
  return (
    <RacButton {...rest} aria-label={label} className={cx("os-btn", "os-btn-tool", "os-icon-btn", className)}>
      {icon}
    </RacButton>
  );
}
