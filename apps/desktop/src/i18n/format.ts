// Zahlen- und Datumsformate nach UI-Sprache (UI-007, UI-121): `Intl`-Instanzen werden gecacht.

const numberCache = new Map<string, Intl.NumberFormat>();
const dateCache = new Map<string, Intl.DateTimeFormat>();

function nf(lang: string, opts: Intl.NumberFormatOptions = {}): Intl.NumberFormat {
  const key = `${lang}|${JSON.stringify(opts)}`;
  let f = numberCache.get(key);
  if (!f) numberCache.set(key, (f = new Intl.NumberFormat(lang, opts)));
  return f;
}

function df(lang: string, opts: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const key = `${lang}|${JSON.stringify(opts)}`;
  let f = dateCache.get(key);
  if (!f) dateCache.set(key, (f = new Intl.DateTimeFormat(lang, opts)));
  return f;
}

export interface Formatters {
  readonly lang: string;
  integer(n: number): string;
  decimal(n: number, digits?: number): string;
  percent(fraction: number, digits?: number): string;
  bytes(n: number): string;
  /** Sekunden → `1:02:03` bzw. `2:03`. */
  duration(seconds: number): string;
  dateTime(date: Date | string | number): string;
  date(date: Date | string | number): string;
  time(date: Date | string | number): string;
}

const asDate = (d: Date | string | number) => (d instanceof Date ? d : new Date(d));

export function makeFormatters(lang: string): Formatters {
  return {
    lang,
    integer: (n) => nf(lang, { maximumFractionDigits: 0 }).format(n),
    decimal: (n, digits = 1) => nf(lang, { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(n),
    percent: (fraction, digits = 0) => nf(lang, { style: "percent", minimumFractionDigits: digits, maximumFractionDigits: digits }).format(fraction),
    bytes(n) {
      const units = ["B", "KB", "MB", "GB", "TB"];
      let v = n;
      let i = 0;
      while (Math.abs(v) >= 1000 && i < units.length - 1) {
        v /= 1000;
        i++;
      }
      const digits = i === 0 ? 0 : v < 10 ? 2 : v < 100 ? 1 : 0;
      return `${nf(lang, { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(v)} ${units[i]}`;
    },
    duration(seconds) {
      const s = Math.max(0, Math.round(seconds));
      const h = Math.floor(s / 3600);
      const m = Math.floor((s % 3600) / 60);
      const sec = s % 60;
      const p = (x: number) => String(x).padStart(2, "0");
      return h > 0 ? `${h}:${p(m)}:${p(sec)}` : `${m}:${p(sec)}`;
    },
    dateTime: (d) => df(lang, { dateStyle: "medium", timeStyle: "medium" }).format(asDate(d)),
    date: (d) => df(lang, { dateStyle: "medium" }).format(asDate(d)),
    time: (d) => df(lang, { timeStyle: "medium" }).format(asDate(d)),
  };
}
