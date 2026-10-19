"""Independent reference for TECH-145; stdlib only, fixed seeds, no network."""
import json
import random
from pathlib import Path

graphs = []
for seed in range(5):
    rng = random.Random(seed)
    n = 30
    edges = [(rng.randrange(1, n + 1), rng.randrange(1, n + 1), rng.random() < 0.7) for _ in range(150)]
    targets = [set() for _ in range(n)]
    follows = [set() for _ in range(n)]
    for s, t, follow in edges:
        if s != t:
            targets[s - 1].add(t - 1)
            if follow:
                follows[s - 1].add(t - 1)
    ranks = [1 / n] * n
    for _ in range(10):
        result = [0.15 / n] * n
        for s in range(n):
            for t in sorted(follows[s]):
                result[t] += 0.85 * ranks[s] / len(targets[s])
        ranks = result
    graphs.append(dict(edges=edges, ranks=ranks))
Path(__file__).with_name('random-ranks.json').write_text('[\n' + ',\n'.join(json.dumps(graph, separators=(',', ':')) for graph in graphs) + '\n]\n')
