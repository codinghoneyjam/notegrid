MODULE R03 bank
  LANG rust ; PHASE P3 ; CRATES { ng-bank }
  DEPENDS_ON { ng-core } ; DEPENDED_BY { ng-synth, ng-wasm }
  PURPOSE SF2/SF3 -> SoundbankImage ; load image ; preset lookup
  OWNS     { RIFF sfbk parser, SF3 sample decode, pre-flattened zones, NGSB image, preset lookup + coverage report }
  NOT_OWNS { sound generation(R04) }
  DESIGN
    parse RIFF sfbk: INFO, sdta, pdta (phdr pbag pmod pgen inst ibag imod igen shdr)
    SF3 = Ogg-Vorbis-compressed samples: decode to i16 PCM at parse time with a pure-Rust Vorbis decoder (choice in spike:SP-4 [VERIFY]) ; 24-bit sm24 ignored in v1
    pre-flatten: per preset merge preset-zone + instrument-zone into resolved zones {key_lo/hi, vel_lo/hi, sample_idx, root_key, tune_cents, atten_cB, pan,
                 vol_env{delay,attack,hold,decay,sustain,release in seconds}, loop_mode, loop_start/end (absolute), exclusive_class, reverb_send} ; v2 adds filter, mod env, LFO, chorus
                 merge semantics per SF2 spec: preset level additive, instrument level overriding [VERIFY against spec]
    image NGSB: versioned, little-endian, aligned sample blob ; Soundbank::from_image = minimal copy, no parsing
    lookup preset(bank, program) fallback chain: melodic (0,p) -> (0,0) ; drum (128,kit) -> (128,0) ; coverage() reports present GM 0-127 and kits
    limits C10.2 ; errors E_BANK_FORMAT | E_BANK_UNSUPPORTED
    v1 generators: key/vel range, root key, coarse/fine/scale tuning, initial attenuation, pan, volume envelope (6 stages), sample modes (none, continuous loop, loop-until-release),
                   start/end/loop offsets, exclusiveClass, reverbSend
  REQUIRE
    R03.R01 never panics (truncation, bad offsets, cycles)
    R03.R02 image build deterministic ; from_image validates version and ranges
    R03.R03 test soundfonts are generated in code (sf2_builder: sine sample, a few presets) — no binary fixtures, no licence issue
  ACCEPT
    R03.AC1 builder SF2 -> image -> presets/zones/samples equal the input
    R03.AC2 same SF2 converted twice -> identical image bytes
    R03.AC3 fuzz (1e5 truncations, 1e5 bit flips) -> 0 panics, bounded memory
    R03.AC4 a real GM SF2 supplied by the user: coverage() lists all 128 programs + kits
    R03.AC5 cached 64 MB image loads <= 1 s [MEASURE]
  RISK SF3 decode memory expansion (tens of MB -> hundreds) ; mitigation: spike:SP-4, recommended image <= 64 MB
END
