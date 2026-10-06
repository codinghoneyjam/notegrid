# Golden changes

Format: date | files | reason | impact

2026-10-06 | `spec/fixture/golden/schedule/example_44100.ngsc` | T1.1 first golden: schedule bytes for `spec/fixture/example_project.json` at 44100 (C3 container, 4 lanes, 1 tempo segment, 20 notes) | None. New baseline for R02.AC4; compiled by `cargo test -p ng-timeline --test ac4_compile`.

2026-10-06 | `spec/fixture/golden/render/example_44100.wav` | T1.6 first render golden: headless render of `spec/fixture/example_project.json` via OscSynth->Renderer->SoftLimiter->WavEncoder, 16-bit PCM | None. New baseline for g05 (native == wasm == golden); produced by `ng-cli render --depth 16`.
