import { useEffect, useState } from "react";

/** Verzögert einen Wert (Tester rechnen erst, wenn die Eingabe kurz ruht). */
export function useDebouncedValue<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  const key = JSON.stringify(value);
  useEffect(() => {
    const id = setTimeout(() => setV(value), ms);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- Vergleich über den serialisierten Wert
  }, [key, ms]);
  return v;
}
