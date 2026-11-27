window.BENCHMARK_DATA = {
  "lastUpdate": 1795804897362,
  "repoUrl": "https://github.com/CarlesClimentOltra/LivingSimulator",
  "entries": {
    "Living Simulator": [
      {
        "commit": {
          "author": {
            "email": "98279159+CarlesClimentOltra@users.noreply.github.com",
            "name": "CarlesClimentOltra",
            "username": "CarlesClimentOltra"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b433a708057fc7f837fcda1301a349cb8959ae63",
          "message": "Merge pull request #5 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nPhase 3: life cycle (friends, couples, births, adoption, schooling, illness, death, migration, commuting)",
          "timestamp": "2026-11-27T19:39:14+01:00",
          "tree_id": "f2cabd3de0828c8e3832c197f96a3acd9d7d19ea",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/b433a708057fc7f837fcda1301a349cb8959ae63"
        },
        "date": 1795804897007,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 245.1,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 168.8,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.6,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1643.9,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1270.5,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1369.7,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.6,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 12004.1,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 14161.9,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 1000000 agents",
            "value": 15.6,
            "unit": "events"
          },
          {
            "name": "memory per agent (columns + journal)",
            "value": 300,
            "unit": "bytes"
          },
          {
            "name": "WASM catch-up, 1 day, region 0 (10000 agents)",
            "value": 386,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 5,
            "unit": "ms"
          }
        ]
      }
    ]
  }
}