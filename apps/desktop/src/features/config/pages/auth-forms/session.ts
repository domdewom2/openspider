import { auth } from "../../../../ipc/auth";
export interface FormLogin { login_url: string; profile: string | null; captured_at: string | null; cookie_count: number }
export interface LoginStatus { profile?: string | null; loginUrl: string; state: "active" | "expired" | "lost" | "missing"; cookies: number; capturedAt: string | null }
export const loginStatus = async () => await auth.loginStatus() as unknown as LoginStatus[];
export const guardDefaults = [
  "(?i).*(log-?out|sign-?out|abmelden|ausloggen).*",
  "(?i).*(delete|remove|loeschen|löschen|entfernen).*",
  "(?i).*/(admin|wp-admin|backend)/.*",
  "(?i).*[?&](action|do)=(delete|remove|logout).*",
];
export function needsGuardConfirmation(config: unknown): boolean {
  const auth = (config as { auth?: { form?: FormLogin[]; session_guard?: string[] } } | null)?.auth;
  return !!auth?.form?.some((f) => !!f.profile) && !auth.session_guard?.some((p) => p.trim());
}
