/** Klassennamen verbinden, Falsches auslassen. */
export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}
