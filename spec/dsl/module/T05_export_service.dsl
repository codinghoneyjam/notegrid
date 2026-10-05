MODULE T05 export_service
  LANG ts(+c wasm for vorbis) ; PHASE P1(WAV) P4(MIDI, OGG, loop, sinks) ; PACKAGES { export-service } ; VENDOR { vendor/vorbis }
  DEPENDS_ON { abi } ; DEPENDED_BY { app }
  PURPOSE export orchestration: offline render -> encoder -> OutputSink ; registries of AudioEncoder and OutputSink
  OWNS     { offline render driver, AudioEncoder adapters (WAV via abi, OGG via libvorbis wasm), OutputSink impls (DownloadSink, DirectorySink), loop sidecar writer }
  NOT_OWNS { render algorithm(R05), WAV/MIDI bytes(R06), filesystem CLI sink(ng-cli) }
  DESIGN
    FLOW ExportRequest{format, sampleRate, bitDepth|quality, loop:'none'|'tail_fold', includeMuted}
      offline worker: ng_offline_begin -> ng_offline_step* (progress, cancel) -> ng_offline_finish -> PCM blob
      encoder.encode(pcm, meta{title, loop_points}, opts) : wav = ng_encode_wav in the same worker ; ogg = PCM streamed to ogg_worker
      OutputSink.write(name, bytes, {overwrite}) with names normalised per C12 ; sidecar <name>.loop.json when looped
      MIDI: main-instance ng_export_midi -> show warnings (W_MIDI_MULTI_PORT ...) -> OutputSink.write
      loop: tail_fold + loop_points ; file length == loop end (R05)
    OGG module: libogg + libvorbis + vorbisenc built with Emscripten as vorbis_enc.wasm, lazy-loaded, cached ; adopting a ready-made wasm encoder package decided in spike:SP-5 [VERIFY: bundle size, streaming input, VBR quality exposed, licence]
      init(channels, sampleRate, quality, comments) -> write(planar f32 chunk of 8192 frames)* -> finish():Uint8Array
      licences of libogg/libvorbis are BSD-style (attribution needed) [VERIFY] -> shown on the open-source notices screen
  REQUIRE
    T05.R01 UI thread never blocked during export (all heavy work in workers)
    T05.R02 AbortSignal cancels at the next progress callback
    T05.R03 registering an encoder or sink makes it appear in the export dialog without UI code changes
    T05.R04 WAV/OGG export uses the same Synth and bank image as preview (D01)
    T05.R05 offline WAV is byte-identical to the native CLI result (R-RT-4)
    T05.R06 all writes go through OutputSink and C12 (names, overwrite policy, atomic/abort)
  ACCEPT
    T05.AC1 example project WAV bytes == `ng-cli render` bytes
    T05.AC2 OGG opens in an independent decoder (CI ffprobe/oggdec) ; length, rate, channels match ; decoded SNR >= threshold (threshold set in P4)
    T05.AC3 looped export: file length == loop end sample ; sidecar values consistent ; WAV smpl parses (when enabled)
    T05.AC4 cancel mid-render leaves no partial file
    T05.AC5 Godot 4.8-dev6 manual checklist: OGG loops seamlessly with only the import-dock Loop (loop start 0) ; with an intro, Loop Offset works ; WAV smpl path checked separately
    T05.AC6 hostile names ("../x", "C:\\x", "CON", 300-char) rejected by every sink with E_PATH
END
