# Domain Memory: Core Platform (ng-core, ng-score, ng-timeline)

Status: EMPTY (starts blank after the 2026-10-07 migration)

Scope:
- ng-core: primitives (Tick, LaneId, Slot), base errors, traits (Synth, AudioEncoder), PcmBuffer.
- ng-score: project model, validation (I-01..I-10, W-1..W-3), canonical JSON, migration, GM tables.
- ng-timeline: TempoMap, compile(project, sr) -> Schedule, schedule (de)serialisation.

Rule: record only verified facts, each with a source (file:line, command output, or test).
