# Domain Memory: Audio Signal Path (ng-dsp, ng-synth, ng-render)

Status: EMPTY (starts blank after the 2026-10-07 migration)

Scope:
- ng-dsp: Envelope, Ramp, interpolation, filters, Reverb, SoftLimiter, seeded Lcg.
- ng-synth: OscSynth (later SoundFontSynth) behind the Synth trait.
- ng-render: Renderer, render_offline, master bus (trait-only knowledge of Synth).
- Realtime rules R-RT-1..4: zero allocation in the render path, no panic,
  denormal protection, determinism (libm, no clock, seeded noise).

Rule: record only verified facts, each with a source (file:line, command output, or test).
