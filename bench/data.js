window.BENCHMARK_DATA = {
  "lastUpdate": 1795861898122,
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
      },
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
          "id": "49c587205591760320d79eaafbc780f8cb55c9e3",
          "message": "Merge pull request #6 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nFix: stable content references in snapshots (resets the live world) + commuting messages",
          "timestamp": "2026-11-27T22:04:09+01:00",
          "tree_id": "054687beeecd5bbe4e0b0ac585fe69f5e6e9fd98",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/49c587205591760320d79eaafbc780f8cb55c9e3"
        },
        "date": 1795813567940,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 251.9,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 169.8,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.6,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1644.9,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1298.8,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1322,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.6,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 12206.2,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 13503.5,
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
            "value": 379,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 5,
            "unit": "ms"
          }
        ]
      },
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
          "id": "8642ace811ae52c52bf7d6beefd19ba99f8dc9b3",
          "message": "Merge pull request #7 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nPhase 4: construction and growth",
          "timestamp": "2026-11-28T08:53:32+01:00",
          "tree_id": "9bff789cb225e7424ef17fc4e6d1b7a5f679d55e",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/8642ace811ae52c52bf7d6beefd19ba99f8dc9b3"
        },
        "date": 1795852554918,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 314.2,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 187.7,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.8,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1651,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1561.9,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1463.8,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 15212.3,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 15075.6,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 1000000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "memory per agent (columns + journal)",
            "value": 300,
            "unit": "bytes"
          },
          {
            "name": "WASM catch-up, 1 day, region 0 (10001 agents)",
            "value": 467,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 7,
            "unit": "ms"
          }
        ]
      },
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
          "id": "8a2732fe991bb5688ccd248a5b018fa3705f0a14",
          "message": "Merge pull request #8 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nPhase 5: crime and justice",
          "timestamp": "2026-11-28T09:39:21+01:00",
          "tree_id": "78bc191483ee9e9a1b0bfffea25798c81c5b9b06",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/8a2732fe991bb5688ccd248a5b018fa3705f0a14"
        },
        "date": 1795855311323,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 311.9,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 188.9,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.8,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1651.8,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1749.8,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1541.4,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 14590.1,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 15095.9,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 1000000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "memory per agent (columns + journal)",
            "value": 300,
            "unit": "bytes"
          },
          {
            "name": "WASM catch-up, 1 day, region 0 (10001 agents)",
            "value": 466,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 8,
            "unit": "ms"
          }
        ]
      },
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
          "id": "1805c786b86a55af4ed2706c853e81f759a57595",
          "message": "Merge pull request #9 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nPhase 6: new towns and the world map",
          "timestamp": "2026-11-28T10:54:56+01:00",
          "tree_id": "7fb04a3dcb3d20904e4f6bb6a7a31a5029c57f4f",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/1805c786b86a55af4ed2706c853e81f759a57595"
        },
        "date": 1795859849836,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 320.8,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 194.3,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.8,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1651.8,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1556.2,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1531.7,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 14833.9,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 15503.3,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 1000000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "memory per agent (columns + journal)",
            "value": 300,
            "unit": "bytes"
          },
          {
            "name": "WASM catch-up, 1 day, region 0 (10001 agents)",
            "value": 480,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 7,
            "unit": "ms"
          }
        ]
      },
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
          "id": "f094fb9ae67d2b023b4f1613790c849c9b4f57ed",
          "message": "Merge pull request #10 from CarlesClimentOltra/claude/friendly-davinci-32gdj1\n\nPhase 7: technology",
          "timestamp": "2026-11-28T11:29:18+01:00",
          "tree_id": "e5cdfd35dacc05cc2dcac2699a95be41f3b4ae58",
          "url": "https://github.com/CarlesClimentOltra/LivingSimulator/commit/f094fb9ae67d2b023b4f1613790c849c9b4f57ed"
        },
        "date": 1795861897529,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "1 simulated day, 10000 agents",
            "value": 263.3,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 10000 agents",
            "value": 177.2,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 10000 agents",
            "value": 15.8,
            "unit": "events"
          },
          {
            "name": "region checkpoint size, 10k agents",
            "value": 1651.8,
            "unit": "KB"
          },
          {
            "name": "1 simulated day, 100000 agents",
            "value": 1328.9,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 100000 agents",
            "value": 1428.7,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 100000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "1 simulated day, 1000000 agents",
            "value": 12629.8,
            "unit": "ms"
          },
          {
            "name": "generation (incl. warm-up), 1000000 agents",
            "value": 14885.1,
            "unit": "ms"
          },
          {
            "name": "events per agent per day, 1000000 agents",
            "value": 15.7,
            "unit": "events"
          },
          {
            "name": "memory per agent (columns + journal)",
            "value": 300,
            "unit": "bytes"
          },
          {
            "name": "WASM catch-up, 1 day, region 0 (10001 agents)",
            "value": 404,
            "unit": "ms"
          },
          {
            "name": "WASM catch-up, 1 day, region 1 (500 agents)",
            "value": 6,
            "unit": "ms"
          }
        ]
      }
    ]
  }
}