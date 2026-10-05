MODULE T07 bank_source
  LANG ts ; PHASE P3 ; PACKAGES { bank-source }
  DEPENDS_ON { abi(types) } FORBIDDEN { playback, persistence, ui-grid } ; DEPENDED_BY { app }
  PURPOSE obtain SF2/SF3 bytes from outside; parsing = R03, caching = T04
  OWNS     { SoundbankSource implementations: UrlSource, LocalFileSource ; link presets ; integrity checks }
  NOT_OWNS { parsing, caching, playback }
  DESIGN
    FLOW user picks a source -> source.acquire() -> bytes -> magic check ("RIFF".."sfbk") -> sha256 -> T04 cache lookup (hit: load image ; miss: bank_worker parse -> store -> load)
         -> remember last bank (source id + reference + sha256) so later starts restore from cache with NO network
    UrlSource (primary): any HTTPS URL entered by the user ; fetch streaming with progress + cancel ; optional pinned expected sha256 ;
         requires the host to send CORS headers and to serve a direct download ; failure -> E_BANK_FETCH with the hint "host not CORS-enabled: download the file and use Local file"
         link presets: saved list {name, url, sha256?, note} kept locally ; http only for localhost development
    LocalFileSource (fallback): <input type=file> + drag-and-drop ; Chromium may keep the picker handle for convenience
    SECURITY downloaded bytes are never executed, only handed to the parser ; size cap 256 MB (warn 64 MB) ; magic mismatch -> E_BANK_FORMAT
    selection screen shows file name, size, sha256 and a notice: "check the soundfont licence before redistributing or re-hosting" (not legal advice)
    extension: any new source = one more SoundbankSource implementation
  REQUIRE
    T07.R01 every source implements the same interface ; UI iterates the registry, never branches on source kind
    T07.R02 downloads are cancellable ; a cancelled download leaves nothing in the cache
    T07.R03 errors carry a code (E_BANK_FETCH | E_BANK_FORMAT) plus user-facing remedy text
    T07.R04 package never imports playback or persistence (wiring in app)
  ACCEPT
    T07.AC1 local file -> bank loaded -> restart with network blocked -> restored from cache (Playwright, 3 engines)
    T07.AC2 UrlSource: succeeds against a CORS-enabled test server ; against a non-CORS server fails with E_BANK_FETCH and the hint
    T07.AC3 cancel leaves the cache untouched
    T07.AC4 fake source injected: UI flow integration test without a browser
END
