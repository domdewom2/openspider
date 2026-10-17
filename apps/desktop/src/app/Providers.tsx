import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

/** TanStack Query für Abfragedaten (Filterzähler, Overview, Issues …); Schlüssel tragen die Kern-Versionen (2.2). */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { staleTime: Infinity, gcTime: 5 * 60_000, refetchOnWindowFocus: false, retry: false },
    },
  });
}

export function Providers({ children, queryClient }: { children: ReactNode; queryClient?: QueryClient }) {
  const [client] = useState(() => queryClient ?? createQueryClient());
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
