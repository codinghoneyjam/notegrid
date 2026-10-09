# CODEBASE ANALYSIS SKILL (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "CODEBASE_ANALYSIS_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. PURPOSE

This skill defines the codebase analysis tooling and workflows for NoteGrid's
Rust (crates/) + TypeScript (packages/) stack. It covers dependency analysis,
dead code detection, architecture conformance, and visualization.

## 2. RUST ANALYSIS TOOLS

### 2.1 cargo-deny (INSTALLED 2026-10-07)
- **What**: Lints dependency graph for licenses, advisories, banned crates, sources
- **Config**: `deny.toml` (already in repo root)
- **Commands**:
  - `cargo deny check licenses` - verify all crate licenses are acceptable
  - `cargo deny check bans` - detect banned/duplicate crates
  - `cargo deny check advisories` - scan for security vulnerabilities
  - `cargo deny check sources` - ensure crates come from trusted sources
  - `cargo deny check` - run all checks
- **When**: CI gate, pre-commit, dependency updates

### 2.2 cargo-udeps (OPTIONAL - needs nightly)
- **What**: Finds unused dependencies in Cargo.toml
- **Install**: `cargo install cargo-udeps`
- **Command**: `cargo +nightly udeps --all-targets`
- **When**: Periodic cleanup, pre-release audit

### 2.3 cargo-tree (BUILT-IN)
- **What**: Visualizes dependency tree
- **Commands**:
  - `cargo tree` - full dependency tree
  - `cargo tree -p ng-core` - single crate tree
  - `cargo tree -i <crate>` - reverse dependencies (who depends on X)
  - `cargo tree --duplicates` - show duplicate versions
- **When**: Debugging dependency bloat, understanding transitive deps

### 2.4 cargo-expand (OPTIONAL)
- **What**: Expands macros to see generated code
- **Install**: `cargo install cargo-expand`
- **Command**: `cargo expand -p ng-synth`
- **When**: Debugging macro-heavy code, understanding trait impls

### 2.5 clippy (BUILT-IN)
- **What**: Rust linter with 400+ lints
- **Command**: `cargo clippy --workspace --all-targets -- -D warnings`
- **When**: CI gate, pre-commit (already in V_GATE)

### 2.6 rust-analyzer (EDITOR)
- **What**: IDE-powered code analysis (goto-def, find-refs, type inference)
- **When**: Interactive development, refactoring

## 3. TYPESCRIPT ANALYSIS TOOLS

### 3.1 dependency-cruiser (INSTALLED 2026-10-07)
- **What**: Validates and visualizes dependencies with custom rules
- **Config**: `.dependency-cruiser.cjs` (already in repo root)
- **Commands**:
  - `npx depcruise src` - validate against rules
  - `npx depcruise --include-only "^src" --output-type dot | dot -T svg > graph.svg` - visualize
  - `npx depcruise --output-type err` - CI-friendly error output
- **When**: CI gate, architecture conformance, visualization

### 3.2 knip (RECOMMENDED)
- **What**: Finds unused files, dependencies, and exports
- **Install**: `pnpm add -D knip`
- **Commands**:
  - `npx knip` - full analysis
  - `npx knip --production` - production code only (no tests/devDeps)
  - `npx knip --workspace packages/app` - single workspace
  - `npx knip --include files,dependencies` - specific issue types
- **When**: Pre-release audit, dead code cleanup

### 3.3 madge (OPTIONAL)
- **What**: Generates visual dependency graphs, finds circular deps
- **Install**: `pnpm add -D madge`
- **Commands**:
  - `npx madge --circular src` - find circular dependencies
  - `npx madge --image graph.svg src` - generate SVG graph
  - `npx madge --orphans src` - find orphan modules
- **When**: Visual debugging, circular dependency detection

### 3.4 ts-prune (OPTIONAL)
- **What**: Finds unused exports in TypeScript
- **Install**: `pnpm add -D ts-prune`
- **Command**: `npx ts-prune src`
- **When**: API surface audit, dead code cleanup

## 4. NOTEGRID-SPECIFIC ANALYSIS

### 4.1 Custom Scripts (ALREADY IN SCRIPTS/)
- `scripts/check_deps.py` - dependency graph conformance (gate g03)
- `scripts/check_decoupling.py` - cross-crate coupling detection
- `scripts/check_hotpath_lints.py` - HOTPATH lint enforcement (gate g01)
- `scripts/check_determinism.py` - determinism verification (gate g05)
- `scripts/gen_tempo_vectors.py --check` - second-opinion vector validation

### 4.2 Architecture Conformance
- **Crate graph**: `spec/dsl/01_system.dsl` (SSOT)
- **Contracts**: `spec/dsl/02_contracts.dsl` (C0..C12)
- **Gate**: `python scripts/check_deps.py` (declared == actual)

### 4.3 Workspace Graph
- **Command**: `cargo metadata --no-deps --format-version 1`
- **When**: Understanding crate topology, debugging build issues

## 5. RECOMMENDED WORKFLOWS

### 5.1 Pre-Commit Analysis
```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
python scripts/check_deps.py
python scripts/check_hotpath_lints.py
npx depcruise src
```

### 5.2 Pre-Release Audit
```powershell
cargo deny check
cargo tree --duplicates
npx knip --production
npx madge --circular src
```

### 5.3 Dependency Update Review
```powershell
cargo tree -i <updated-crate>
cargo deny check advisories
cargo udeps --all-targets  # if nightly available
```

### 5.4 Architecture Visualization
```powershell
cargo tree --workspace --dot | dot -T svg > docs/dependency-graph.svg
npx depcruise --include-only "^src" --output-type dot | dot -T svg > docs/ts-dependency-graph.svg
```

## 6. TOOL SELECTION GUIDE

| Need | Tool | Already Installed |
|---|---|---|
| License compliance | cargo-deny | YES |
| Security advisories | cargo-deny | YES |
| Unused Rust deps | cargo-udeps | NO (nightly) |
| Rust dep tree | cargo-tree | YES |
| TS dependency rules | dependency-cruiser | YES |
| Unused TS files/exports | knip | NO |
| Circular TS deps | madge | NO |
| Unused TS exports | ts-prune | NO |
| Architecture conformance | scripts/check_deps.py | YES |
| HOTPATH lint check | scripts/check_hotpath_lints.py | YES |
| Determinism check | scripts/check_determinism.py | YES |

## 7. INSTALLATION COMMANDS

```powershell
# Rust tools
cargo install --locked cargo-deny
cargo install cargo-udeps  # requires nightly
cargo install cargo-expand

# TypeScript tools
pnpm add -D knip
pnpm add -D madge
pnpm add -D ts-prune
```

## 8. REFERENCES
- cargo-deny: https://github.com/EmbarkStudios/cargo-deny
- cargo-udeps: https://github.com/est31/cargo-udeps
- dependency-cruiser: https://github.com/sverweij/dependency-cruiser
- knip: https://github.com/webpro/knip
- madge: https://github.com/pahen/madge
- ts-prune: https://github.com/nadeesha/ts-prune
