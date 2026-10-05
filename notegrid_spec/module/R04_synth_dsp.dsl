MODULE R04 synth_dsp
  LANG rust ; PHASE P1(Osc, DSP) P3(SoundFont) ; CRATES { ng-dsp, ng-synth }
  DEPENDS_ON { ng-dsp: { ng-core } ; ng-synth: { ng-core, ng-dsp, ng-bank } } ; DEPENDED_BY { ng-wasm, ng-cli }
  PURPOSE DSP parts and Synth implementations
  OWNS     { Envelope(DAHDSR), Ramp(5 ms), LinearInterp(v2 cubic), Biquad/OnePole(v2), Reverb(8 combs + 4 allpass, stereo offsets), SoftLimiter, Lcg, OscSynth, SoundFontSynth }
  NOT_OWNS { scheduling/transport(R05), parsing(R03) }
  DESIGN
    libm for sin/exp/pow/log ; feedback paths protected against denormals
    OscSynth: fixed voice pool (default 64, no allocation after creation) ; GM 16 families -> (waveform, envelope, light detune) table ; purpose = instant sound + test baseline, not fidelity
              drums: key ranges -> kick(sine sweep), snare/hat/clap(noise+band), toms(sine sweeps), cymbals(long noise) ; hat open/closed exclusive
              voice stealing: oldest voice in release -> else quietest -> ties by slot number (deterministic)
    SoundFontSynth: preset lookup(R03) ; every zone matching (key, vel) sounds (layers) ; linear interpolation, loop modes, volume envelope, attenuation/pan/tuning,
                    velocity->attenuation per SF2 default modulator [VERIFY formula against spec], exclusiveClass choke, reverbSend -> send bus ;
                    set_program applies from the next note_on
    GATE spike:SP-3 build-vs-adopt: adopt an existing SF2 synth crate behind a Synth adapter iff ALL of
         { zero allocation in render path ; bit-exact determinism with libm pinned ; wasm size within budget ; exclusiveClass + loop modes + >=129 channel mix control ; licence compatible }
         else build ; SHOULD { SF3, filter/LFO } ; candidate behaviour is [VERIFY]
    either way the same synth_contract tests must pass (L)
  REQUIRE
    R04.R01 synth_contract<S: Synth>(make) : idle output exactly 0 ; note_on -> energy > 0 ; note_off -> below -60 dB within release time ; gain 0 -> silence ; pan centre L==R, hard-left R~0 (constant power) ;
            two instances same input -> bit-identical ; 10 s random events -> no NaN/Inf ; voice cap respected ; silence_all -> zeros within N samples
    R04.R02 render path: zero allocation (counting allocator) ; clippy hotpath deny
    R04.R03 voice pool size set by RenderConfig ; overflow = stealing, not error
    R04.R04 Reverb output flushes to exactly 0 after input ends (below -120 dBFS)
  ACCEPT
    R04.AC1 OscSynth passes all synth_contract items
    R04.AC2 128 programs x 3 keys rendered by OscSynth: all non-silent, no NaN
    R04.AC3 (P3) SoundFontSynth passes synth_contract ; builder-SF2 sine pitch error <= 1 cent
    R04.AC4 (P3) real GM SF2: 128 programs + Standard kit all audible (0 silent presets) ; golden hashes stored
    R04.AC5 64-voice block cost <= BUDGET.worklet_cost [MEASURE]
  RISK SoundFont fidelity vs schedule (filter/LFO deferred; assumption that most GM presets are acceptable without them [VERIFY by listening]) ; reverb tuning = data table, no code change
END
