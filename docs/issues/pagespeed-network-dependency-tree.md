# PageSpeed: Network Dependency Tree

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Kritische Ressourcen hängen in langen Ketten voneinander ab, sodass jede erst nach der vorherigen angefordert wird.

**Warum ist es relevant?** Lange Anforderungsketten summieren Netzwerklatenzen und verzögern die Darstellung.

**Wie beheben?** Verkürze die Ketten, lade wichtige Ressourcen früh vor und baue Verbindungen zu benötigten Hosts vorab auf (preconnect).

## English

**What does it mean?** Critical resources depend on each other in long chains, so each is requested only after the previous one.

**Why does it matter?** Long request chains add up network latency and delay rendering.

**How to fix** Shorten the chains, preload important resources and open connections to required hosts early (preconnect).
