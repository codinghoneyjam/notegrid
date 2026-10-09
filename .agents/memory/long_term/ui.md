# Domain Memory: UI Packages (abi, document, editing, ui-grid, playback, persistence, app)

Status: EMPTY (starts blank after the 2026-10-07 migration)

Scope:
- abi: wasm loader, typed ABI wrapper, schema types, GM JSON.
- document: document model, immutable updates, queries.
- editing: commands, history, selection, snap, quantize, intent handling.
- ui-grid: canvas rendering, viewport, hit-test, input -> Intent.
- playback: worklet AudioEngine, schedule sync policy.
- persistence: project store, autosave, bank image cache.
- app: composition root, layout, PWA, settings (only place that imports implementations).

Rule: record only verified facts, each with a source (file:line, command output, or test).
