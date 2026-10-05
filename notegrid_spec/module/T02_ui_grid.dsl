MODULE T02 ui_grid
  LANG ts ; PHASE P2 ; PACKAGES { ui-grid }
  DEPENDS_ON { document(readonly) } FORBIDDEN { editing, playback } ; DEPENDED_BY { app }
  PURPOSE draw the time x lane grid and translate pointer/keyboard input into Intents ; never mutates the document
  OWNS     { canvas rendering, viewport, culling, hit-test, lane header, program palette, keymap }
  NOT_OWNS { edit rules(T01), sound(T03) }
  DESIGN
    layout: ruler (bars, beats, tempo/meter markers, loop bracket, playhead) over lane rows ; lane header = fold toggle, colour, name, instrument chip (opens palette), M/S, volume/pan
    program palette: 16 GM families -> 128 programs, searchable ; click = set selected lane instrument ; drag onto grid = new lane ; "all programs" view via EnsureProgramLanes
    lane display modes: Compact (24 px row, notes as bars, pitch as label/shade) | Roll (selected/expanded lane, vertical = pitch, auto range, keyboard ruler) | Drum (rows for used drum keys with names)
    vertical drag of a note to another lane of the same kind = change instrument
    input -> Intent
      click on empty (pencil) -> addNote(last length, snapped) + audition event
      drag note -> moveNotes (Shift: lock pitch, Alt: copy) ; pitch change -> audition (throttle 80 ms)
      right edge drag -> resizeNotes ; box / Shift select / Ctrl+A -> selection event only
      Delete, Ctrl+C/X/V/D, arrows (Shift = octave/bar), Ctrl+Z / Ctrl+Shift+Z, Space ; wheel: Ctrl = x zoom, Alt = y zoom, Shift = horizontal scroll
      Esc during a pointer gesture cancels (no commit)
    keymap = ONE table (keymap.ts) ; help overlay '?' is generated from it
    rendering: 3 canvas layers (static grid | notes | overlay: selection, playhead) ; HiDPI ; culling by per-lane sorted arrays ; dirty-lane redraw ; one rAF per frame ; playhead updates overlay only ; lanes virtualised
  REQUIRE
    T02.R01 document is DeepReadonly here ; no editing/playback imports
    T02.R02 >= 30 fps with 5000 visible notes [MEASURE] ; playhead motion never redraws the notes layer
    T02.R03 every primary action reachable by keyboard
    T02.R04 128 lanes scroll and draw within budget (only visible lanes drawn)
    T02.R05 Esc cancels gestures without committing
  ACCEPT
    T02.AC1 scenario: 4 lanes x 16 bars composed without touching JSON ; moving notes across lanes changes the instrument
    T02.AC2 one drag == one undo step (integration with T01)
    T02.AC3 128-lane document: scroll fps within budget ; draw-call count proportional to visible lanes
    T02.AC4 add/move/delete/copy/paste by keyboard only
    T02.AC5 ui-grid bundle contains neither editing nor playback (gate:g07)
END
