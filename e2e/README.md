# E2E harness: Playwright across chromium / firefox / webkit.

`node build.mjs` bundles `harness/harness.ts` against the real package sources
(`packages/persistence`, `packages/app`, `packages/document`, `packages/abi`) and serves
the built wasm, so a rename in `src/` breaks the specs instead of leaving them green.
`playwright.config.ts` starts that server for you via `webServer`.

```
pnpm install --ignore-workspace   # e2e is not a pnpm workspace member
pnpm install-browsers             # once per machine
pnpm test
```

The harness page is `harness/harness.html`, which mounts the validation panel and exposes
`window.ng` for the specs. `e2e/harness/harness.js` and `e2e/test-results/` are build and
run artifacts and are gitignored.

## What each spec covers

| spec | covers |
|---|---|
| `persistence.spec.ts` | T04.AC6: save/open on all three engines, with the fallback path as the one under test where File System Access is absent |
| `smoke.spec.ts` | placeholder; gate g10 still needs a worklet that sounds a block |