MODULE T01 editing
  LANG ts ; PHASE P2 ; PACKAGES { document, editing }
  DEPENDS_ON { document: { abi(types) } ; editing: { document } } ; DEPENDED_BY { app }
  PURPOSE document shape and ALL mutations as Commands (undo/redo, selection, clipboard, snap, quantise)
  OWNS     { Document, immutable updates, queries, Command, History, selection, clipboard, snap, quantise, overlap policy, Intent handling }
  NOT_OWNS { drawing(T02), sound(T03), storage(T04) }
  DESIGN
    Document { title, tempo[], meter[], loop|null, lengthTicks|null, lanes[] /*order = screen order*/, notes: ReadonlyMap<LaneId, readonly Note[]> /*start-ascending*/ }
    Note { id:number /*runtime only, never saved*/, start, dur, pitch, vel }
    structural sharing: an edit rebuilds only the affected lanes' arrays ; helpers notesInRange(lane,t0,t1) by binary search, findNote(id), barBoundaries(doc)
    save = strip ids, convert to tuples ; Rust canonical is final authority
    COMMANDS notes { AddNote, MoveNotes(time, pitch, lane move = change instrument), ResizeNotes, DeleteNotes, SetVelocity, Transpose, Quantize, Paste, Duplicate }
             lanes { AddLane(kind,program), RemoveLane(incl. its notes), ReorderLane, SetLaneProgram, SetLaneMix, RenameLane, SetLaneColor }
             song  { SetTempo, SetMeter(snaps to bar boundary), SetLoop, SetLength }
             aux   { EnsureProgramLanes(set of programs -> create missing lanes; basis of the "all programs" view) }
    overlap policy (I-03): new/moved note overlapping an existing same-lane same-pitch note: (1) trim the existing note's end to the new start (2) fully covered -> delete existing (3) same start -> replace ; resolved once on the resulting set
    History: execute/undo/redo ; depth 200 ; continuous gestures (drag, resize) commit as ONE command on pointer-up ; snapshots are document references
    selection and view state live outside the document (not undoable) ; undo drops selected ids that no longer exist
    clipboard 'application/x-ng-notes' {ref_tick, items:[{laneKind,dLane,dTick,dur,pitch,vel}]} ; paste at playhead (or selection start) ; lane-kind mismatch -> refuse with message
    snap 1/1..1/64 + triplets + off ; quantise(grid, strength 0-100%, include length)
  REQUIRE
    T01.R01 no Command yields a document violating I-01..I-10
    T01.R02 Command is pure: apply(doc)->doc, no external state, clock or randomness
    T01.R03 undo/redo <= BUDGET.edit_latency @5000 notes ; single add <= 5 ms [MEASURE]
    T01.R04 editing never imports ui-grid or playback
    T01.R05 MoveNotes toLane refused when lane kinds differ
  ACCEPT
    T01.AC1 fast-check: random command sequences, ng_project_validate reports 0 errors after every step
    T01.AC2 random sequence then undo-all == initial document (deep equal) ; redo-all == final
    T01.AC3 overlap policy: trim / delete / replace each covered by a unit test
    T01.AC4 undo of lane removal restores notes, ids and order
    T01.AC5 meter change snaps to bar boundary ; I-05-violating input refused
END
