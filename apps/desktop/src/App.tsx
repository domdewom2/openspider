import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("app_version")
      .then(setVersion)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  return (
    <main>
      <h1>OpenSpider</h1>
      {version !== null && <p data-testid="version">Version {version}</p>}
      {error !== null && <p role="alert">Fehler: {error}</p>}
    </main>
  );
}
