MODULE R06 export
  LANG rust ; PHASE P1(WAV) P4(MIDI export + import) ; CRATES { ng-export }
  DEPENDS_ON { ng-core, ng-score } ; DEPENDED_BY { ng-wasm, ng-cli }
  PURPOSE WavEncoder (AudioEncoder), MidiExporter (ScoreExporter), MidiImporter (ScoreImporter)
  OWNS     { byte-level WAV and SMF rules (C8) }
  NOT_OWNS { audio rendering(R05), OGG(T05) }
  DESIGN
    WAV: hand-written RIFF ; 16/24 int + 32 float ; clip count ; optional seeded TPDF dither ; smpl chunk iff loop_points ; INAM iff title ; even padding ; > 4 GiB -> E_ENCODE
    MIDI export: per C8 channel plan, multi-port with FF21 ; deterministic ; VLQ delta times
    MIDI import: PPQ != 480 -> round(tick * 480 / ppq) ; lane key = (track, channel) ; program = first Program Change (default 0) ; channel 10 -> drum lane ;
                 same lane+pitch overlap -> trim earlier note + W_IMPORT_TRIMMED ; zero-length notes and velocity-0 note-on handled ; tempo and meter maps restored ;
                 tempo events <= 65_535 ; lanes > 128 -> error ; result must pass validate ; options: no quantise (default) | 16th grid
  REQUIRE
    R06.R01 exporters assume a validated Project; violations -> ExportError (no panic)
    R06.R02 same input -> same bytes
    R06.R03 outputs re-read by independent implementations (WAV: hound-like, MIDI: midly-like dev-dependencies) [VERIFY crate choice]
    R06.R04 20 melodic lanes + 2 drum lanes: no channel collision
  ACCEPT
    R06.AC1 WAV 16/24/float read back within +-1 LSB ; smpl and INFO chunks parse
    R06.AC2 example project -> MIDI -> independent parser: notes, tempo, meter, programs equal
    R06.AC3 multi-port channel plan equals golden ; port meta present
    R06.AC4 export -> import round trip: same note set (lane order/names excluded), same tempo map
    R06.AC5 real players/DAWs open the MIDI (manual checklist recorded)
END
