MODULE T03 playback
  LANG ts(+worklet js) ; PHASE P1(minimal) P2(editing sync) P3(bank) ; PACKAGES { playback }
  DEPENDS_ON { abi } FORBIDDEN { ui-grid } ; DEPENDED_BY { app }
  PURPOSE AudioEngine over an AudioWorklet + the document->schedule synchronisation policy
  OWNS     { AudioContext lifecycle, worklet node, message I/O, onPosition, sync policy, watchdog }
  NOT_OWNS { sound(R04/R05), UI }
  DESIGN
    files: engine.ts (context, node, messages) ; worklet.js (sync wasm instantiate, process() calls ng_engine_render with L/R buffers allocated once, handlers, ~30 Hz position report) ; sync.ts (policy)
    SYNC
      notes | tempo | meter | lane add/remove/reorder | program change -> 50 ms debounce -> ng_compile (main instance) -> setSchedule(transfer) ; tick position preserved (C2.6)
      lane volume/pan/send/mute/solo -> setMix immediately, no recompile ; loop range -> setLoop immediately
      note add / pitch change while editing -> audition on the lane's instrument (~250 ms or while held)
      compile error -> keep the previous schedule, emit error event (playback never breaks mid-edit)
      engine/compile sample rate = AudioContext.sampleRate ; export rate independent
      init() from a user gesture ; suspended context (hidden tab) tolerated ; resume continues
      worklet error or silence > 2 s watchdog -> recreate node, resend last schedule + mix + bank
      loadSoundbank only while stopped (else E_BUSY)
  REQUIRE
    T03.R01 minimal JS allocation in process() ; buffers/views reused
    T03.R02 click -> first sound <= 150 ms (active context) [MEASURE]
    T03.R03 position reports <= 30 Hz ; playhead shows latest value
    T03.R04 playback never imports ui-grid
    T03.R05 schedule/mix/bank message order preserved (single FIFO port)
  ACCEPT
    T03.AC1 headless smoke on 3 engines: worklet loads, a 128-frame block with a schedule is non-silent
    T03.AC2 note added during playback: position jump <= 1 tick, position sequence monotonic
    T03.AC3 compile failure: previous schedule retained + error event
    T03.AC4 killed worklet recovers automatically and restores state
    T03.AC5 bank load refused while playing, accepted when stopped
END
