MODULE R05 renderer
  LANG rust ; PHASE P1 ; CRATES { ng-render }
  DEPENDS_ON { ng-core, ng-timeline, ng-dsp } ; DEPENDED_BY { ng-wasm, ng-cli }
  PURPOSE drive Schedule + Synth into audio blocks ; realtime and offline share one implementation
  OWNS     { Renderer state machine, sub-block splitting, output-clock note_off queue, loop wrap, mix ramps, master bus, render_offline, tail fold }
  NOT_OWNS { voices(R04), encoding(R06), threads/messages(R07,T03) }
  DESIGN
    FSM Renderer: Stopped <-> Playing <-> Paused ; Ended (schedule end and no loop) ; voices keep rendering in every state so tails ring out
    queue: fixed-capacity min-heap array (512) keyed by OUTPUT clock ; full -> force the earliest off, count dropped_offs
    loop: when transport reaches loop_end jump to loop_start ; queue is clock-based so it is unaffected
    mix: effective_gain = any_solo ? (solo ? gain : 0) : (mute ? 0 : gain) ; per-slot Ramp 5 ms
    master: dry + Reverb(send) -> SoftLimiter
    set_schedule: keep tick position (C2.6) ; keep queue and voices
    seek: silence_all (short fade) then move (L-01) ; audition slot works regardless of transport state
    offline: progress callback every 4096 frames, false -> E_RENDER_CANCELED ; solo_lane option renders one lane (stems = repeat per lane)
  REQUIRE
    R05.R01 render_block: zero allocation, no panic
    R05.R02 any block size 1..8192 yields bit-identical output (block-boundary independence)
    R05.R03 same Schedule+Synth+options -> bit-identical offline result native vs wasm
    R05.R04 100 loop iterations: zero missed or duplicated notes (event counters)
    R05.R05 never references OscSynth/SoundFontSynth (dependency gate)
  ACCEPT
    R05.AC1 block sizes {1,7,64,128,4096} -> identical bytes
    R05.AC2 looped project: last note's release energy appears in S..S+tail ; file length == E
    R05.AC3 set_schedule during playback (add note): tick position stays within +-1 tick, sounding notes keep their end times
    R05.AC4 queue-full scenario (note flood beyond voice cap): no panic, no unbounded wait
    R05.AC5 mute/solo toggles are click-free (adjacent-sample difference under threshold)
    R05.AC6 offline cancel stops at the next progress callback
END
