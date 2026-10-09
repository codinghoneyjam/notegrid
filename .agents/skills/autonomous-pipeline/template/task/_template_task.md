---
task_id: "{DOMAIN}-{SLOT}-{name}"
plan_ref: "docs/work/{plan_slug}/plan/{plan_name}.yaml"
f_read: ["crates/ng-dsp/src/lib.rs"]
f_modify: ["crates/ng-dsp/src/biquad.rs"]
line_limit: "Recommended <= 300, Max <= 400 (+10 buffer allowed)"
---

# Operations

- file: "crates/ng-dsp/src/biquad.rs"
  action: modify # add | delete | modify
  target:
    - "fn process(&mut self, x: f32) -> f32"
    - "const Q_MAX: f32"
  spec:
    few_shot:
      - symbol: "const Q_MAX: f32"
        as_is: |
          # None (not declared)
        to_be: |
          const Q_MAX: f32 = 32.0;
      - symbol: "fn process(&mut self, x: f32) -> f32"
        as_is: |
          fn process(&mut self, x: f32) -> f32 {
              x
          }
        to_be: |
          fn process(&mut self, x: f32) -> f32 {
              // Updated implementation logic (libm-routed transcendentals only)
              let cut: f32 = self.cutoff.clamp(0.0, 1.0);
              cut * x
          }

## Verification
- `V_GATE` (.agents/language/toolchain.md)
- role test command the card names: `cargo test -p <crate>` or `pnpm -r test`

## Completion Summary
(To be filled in 100% English ASCII upon completion, before the memory commit)
