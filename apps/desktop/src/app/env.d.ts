/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `ws://127.0.0.1:<port>/?token=<hex>` der E2E-Bridge (nur Testbuilds, docs/plan/M2.md 3.6). */
  readonly VITE_E2E_BRIDGE?: string;
  /** `1` in Testbuilds: erlaubt `?bridge=` und `?transport=mock` in der Adresse. */
  readonly VITE_E2E?: string;
}
