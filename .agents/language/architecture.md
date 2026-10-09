# ONTOLOGY: ARCHITECTURE & COMPOSITION TERMS (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "ARCHITECTURE_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_spec: "spec/dsl/01_system.dsl"
---

## 1. CANONICAL VOCABULARY (ENGLISH SSOT)
* `Crate`: Rust unit under `crates/` (ng-*). One reason to change per crate (RULE S).
* `Package`: TypeScript unit under `packages/`. Abstractions live with the consumer.
* `Assembly`: the artifacts produced from ng-wasm and ng-cli (wasm module, native tool).
* `HOTPATH`: the set { ng-dsp, ng-synth, ng-render }; crate-wide g01 lint opt-in; no local allow.
* `Composition Root`: the only places implementations are injected: the `app` package and the ng-wasm / ng-cli crates.
* `Worklet`: AudioWorklet instance rendering 128-frame blocks (realtime thread rules apply).
* `Engine`: one ng-wasm instance driving Renderer + Synth (worklet or offline worker).
* `Schedule`: the compiled binary project (C3); shared by preview and export.
* `SoundbankImage`: parsed soundbank in a load-cheap binary form.
* `Synth`: trait NoteSink + ChannelMix + AudioSource + Send; implementations: OscSynth, later SoundFontSynth.
* `Renderer`: owns transport and the master chain; knows the Synth trait only, never an implementation.
* `OutputSink`: where exported bytes go (download, picked directory, CLI filesystem inside OWN_IO).
* `Scope Unit`: a crate or package name used by the dispatch predicates (AGENTS.md SCOPE fences).

## 2. DEPENDENCY MEANING
* EDGE direction: consumers own abstractions (the ng-render -> trait Synth <- ng-synth pattern).
* Default deny: every edge not declared in the ARCHITECTURE block of the spec is a violation.
* Graph equality (declared == actual) is a gate, not an aspiration: gate g03.
* A reference is an edge in these kinds: Cargo dependency, TS import, C ABI call.

## 3. DETERMINISM TERMS
* `byte-identical`: native render == wasm render, bit for bit (gate g05).
* `libm-routed`: every transcendental in HOTPATH goes through the pinned libm crate.
* `seeded Lcg`: noise comes from the deterministic generator owned by ng-dsp; never a global RNG.
* `no clock`: wall-clock time and HashMap iteration order are banned in render and export paths.

## 4. REFERENCES
* Topology SSOT: spec/dsl/01_system.dsl (CRATE / PACKAGE / EDGE lines).
* Interface SSOT: spec/dsl/02_contracts.dsl (C0..C12).
* Sets and gates: spec/dsl/00_dsl.dsl.
