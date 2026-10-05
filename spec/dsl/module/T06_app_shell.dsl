MODULE T06 app_shell
  LANG ts ; PHASE P0(skeleton) extended per phase ; PACKAGES { app }
  DEPENDS_ON { ALL packages } ; DEPENDED_BY {}
  PURPOSE composition root: inject implementations into interfaces ; layout ; PWA ; settings
  OWNS     { createApp(deps), layout, service worker, settings, keymap loading }
  NOT_OWNS { any editing/playback/storage/export behaviour }
  DESIGN
    WIRING AudioEngine=WorkletAudioEngine ; ProjectStore=IndexedDbStore+FileStore ; AudioEncoder registry={WavEncoder, OggEncoder} ; SoundbankSource registry={UrlSource, LocalFileSource} ;
           OutputSink registry={DownloadSink, DirectorySink(if available)} ; History/Command=editing ; Ng = one main-thread instance (compile, validate, MIDI)
    createApp(deps) takes dependencies as arguments so tests inject fakes
    layout: top bar (file, undo/redo, transport, BPM/meter, loop, export) | left program palette | centre ui-grid | right inspector | bottom validation + log
    menus: open (.ngproj.json, .mid), save, save as, export (WAV/OGG/MIDI + destination), soundbank manager (source, link presets, cache list/delete, coverage)
    settings: export sample rate, metronome, theme, export destination (stores only the chosen directory handle)
    PWA: manifest + service worker caching app shell, ng-wasm.wasm, vorbis_enc.wasm in a VERSIONED cache, cache-first ; new version -> "update available", applied by the user ; storage persistence requested ; offline indicator
    optional single-file build (wasm base64, worklet via Blob URL) [VERIFY: spike:SP-6] ; direct file:// opening is not the primary path
    browsers per C11 ; required feature missing -> start screen explains ; Safari/PWA install hint and storage warning shown in settings
  REQUIRE
    T06.R01 no package other than app imports another package's implementation
    T06.R02 first visit then network blocked: start, edit, save, WAV/MIDI export work ; OGG needs its module cached (pre-cache option provided)
    T06.R03 errors shown to the user with code ; logged locally (IndexedDB) ; nothing leaves the machine
    T06.R04 keymap has a single source
    T06.R05 desktop Chrome/Edge/Firefox/Safari per C11 ; unsupported features degrade by feature detection with a visible reason
    T06.R06 no game paths or game-specific settings anywhere in code or config (DR-2)
  ACCEPT
    T06.AC1 Playwright offline mode: start -> place notes -> play -> export WAV succeeds
    T06.AC2 fakes injected: edit -> schedule sync -> save integration test passes without a browser
    T06.AC3 service-worker update scenario: new version -> notice -> apply -> old cache cleaned
    T06.AC4 dependency check: zero cross-package implementation imports outside app
    T06.AC5 Chromium, Firefox, WebKit: start -> place -> play -> export WAV passes ; missing features fall back
END
