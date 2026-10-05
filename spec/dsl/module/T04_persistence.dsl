MODULE T04 persistence
  LANG ts ; PHASE P2 ; PACKAGES { persistence }
  DEPENDS_ON { abi } ; DEPENDED_BY { app }
  PURPOSE save/restore projects and bank images
  OWNS     { ProjectStore impls, autosave, recovery, bank image cache, error log }
  NOT_OWNS { format rules (Rust canonical/migrate), output export (T05) }
  DESIGN
    IndexedDB "notegrid": project{id,name,updated,bytes} ; autosave{id,ts,bytes} ring of 5 ; bank{key=sha256 of source, name, size, image, image_version} ; log{last 200 errors}
    files: File System Access pickers when available, else <input type=file> + download ; extension .ngproj.json ; Chromium may keep a directory handle for ROOT/music_source
    open: bytes -> ng_project_migrate -> ng_project_validate (errors: show list, refuse ; warnings: open) -> document
    save: document -> tuples -> ng_project_canonical -> bytes (always canonical)
    autosave: 5 s debounce + immediately when the tab hides ; on start, newer autosave -> recovery dialog
    bank cache: acquire (T07) -> sha256 -> cache lookup -> miss: bank_worker parse -> store image ; image_version change invalidates and re-parses ; cache usable with the original gone
    MIDI open: .mid/.midi -> ng_import_midi -> new project or add lanes to the current one -> show import warnings
    request navigator.storage.persist() ; quota problems surface as E_STORAGE
    SAFARI/inactivity: script-writable storage may be purged after long inactivity [VERIFY] -> (1) persist() request (2) PWA install hint (3) show "last saved to file N days ago", warn at >= 7 days
                       (4) saving the .ngproj.json file is the PRIMARY safety net ; autosave is temporary protection and is labelled so
  REQUIRE
    T04.R01 saved bytes are always canonical (save -> open -> save is byte-identical)
    T04.R02 autosave failure informs the user and never blocks editing
    T04.R03 everything works offline
    T04.R04 FileStore works on browsers without File System Access (fallback)
    T04.R05 bank images are cached independently of the source file
    T04.R06 every E_STORAGE offers "save to file" as the alternative
  ACCEPT
    T04.AC1 save -> open round trip: deep-equal document, file bytes == canonical
    T04.AC2 invalid file: error list (code, path) shown, document unchanged
    T04.AC3 reload -> recovery dialog -> recovery succeeds
    T04.AC4 second load of the same bank via cache <= 1 s [MEASURE]
    T04.AC5 quota-exceeded simulation: editing continues, warning shown
    T04.AC6 Firefox and WebKit projects: save/open via the fallback path pass
END
