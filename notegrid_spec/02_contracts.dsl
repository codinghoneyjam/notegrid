# 02_contracts.dsl — interface contracts. A CONTRACT overrides module text. Change = new DECIDE + contract test + migration.

## C0 UNITS AND CONVENTIONS
RULE C0.1 tick: u32 ; PPQ = 480 fixed
RULE C0.2 tempo stored as mpq (u32 microseconds per quarter) ; JSON `bpm` is display only (3 decimals):
          mpq = round(60_000_000 / bpm) ; saved bpm = round(60_000_000 / mpq, 3) ; load->save->load is stable after first normalisation
RULE C0.3 tick->sample: per tempo segment accumulate sum(delta_tick * mpq) as u128 ; sample = round_half_up(T * sr / (480 * 1_000_000)) ONCE at the end
          EXAMPLE 124 bpm (mpq 483871), 7680 ticks -> 341419 samples @44100, 371613 samples @48000
RULE C0.4 pitch 0-127, C4 = 60 ; velocity 1-127 (0 forbidden) ; lane volume/pan/reverb_send 0-127 integers ; pan 64 = centre, constant-power law
RULE C0.5 internal audio = f32 stereo ; default 44100 Hz (48000 selectable) ; integer conversion only at the very end
RULE C0.6 lane kind: melodic(program 0-127) | drum(kit 0-127 = GM bank-128 program, only 0 = Standard guaranteed) ; drum pitch = GM percussion key (normally 27-87)
RULE C0.7 files/folders snake_case, folders singular ; UTF-8 ; LF

## C1 PROJECT FILE  (JSON, format_version 1)
SOURCE_OF_TRUTH schema/project.schema.json (structure) + invariants below (semantics) ; example fixture/example_project.json
NOTE_ENCODING [lane, start, duration, pitch, velocity] one tuple per line
INVARIANT I-01 lane ids unique, >= 1 ; lanes <= 128                                      -> E_LANE_DUP | E_RANGE
INVARIANT I-02 every note.lane exists                                                    -> E_LANE_MISSING
INVARIANT I-03 same lane + same pitch notes never overlap (end <= next start)            -> E_OVERLAP
INVARIANT I-04 tempo[0].tick == 0 ; ticks strictly increasing                            -> E_TEMPO
INVARIANT I-05 meter[0].tick == 0 ; increasing ; each change sits on a bar boundary of the previous meter -> E_METER
INVARIANT I-06 loop present -> start < end <= effective length                           -> E_LOOP
INVARIANT I-07 effective length = length_ticks if present else bar-ceil(max note end) ; length_ticks < max note end -> W_NOTE_PAST_LENGTH
INVARIANT I-08 start + duration <= u32::MAX                                              -> E_RANGE
INVARIANT I-09 format_version > supported                                                -> E_VERSION_TOO_NEW
INVARIANT I-10 unknown fields: only `x_`-prefixed are preserved, all others                -> E_UNKNOWN_FIELD
INVARIANT W-1  drum key outside 27-87 -> W_DRUM_KEY_RANGE ; W-2 lane without notes -> W_UNUSED_LANE ; W-3 bpm changed by mpq normalisation -> W_BPM_ROUNDED
CANONICAL  key order = schema order ; 2-space indent ; notes sorted (start, lane, pitch), one per line ; LF + final newline ; canonical file load->save is byte-identical
LIMIT      file <= 64 MB ; notes <= 1_000_000 -> E_RANGE

## C2 RUST TRAITS  (owner crate in comment)
API
  // ng-core
  type Tick = u32 ;  struct LaneId(u32) ;  struct Slot(u16) ;  const MAX_SLOTS = 129 ;  const AUDITION_SLOT = Slot(128)
  enum LaneKind { Melodic, Drum }
  trait NoteSink   { set_program(slot, kind, program:u8) ; note_on(slot, key:u8, vel:u8) ; note_off(slot, key:u8) ; release_all() ; silence_all() }
  trait ChannelMix { set_gain(slot, gain:f32 /*linear 0..1, after mute/solo*/) ; set_pan(slot, pan:u8) ; set_reverb_send(slot, send:u8) }
  struct Buses<'a> { dry_l, dry_r, send_l, send_r : &'a mut [f32] }            // equal lengths
  trait AudioSource { sample_rate()->u32 ; render(&mut self, out:&mut Buses)  /*overwrites*/ ; active_voices()->usize }
  trait Synth : NoteSink + ChannelMix + AudioSource + Send
  struct PcmBuffer { sample_rate:u32, channels:u8, frames:usize, data:Vec<f32> /*interleaved*/ }
  struct EncodeMeta { title:Option<String>, loop_points:Option<(u32,u32)> /*samples [start,end)*/ }
  trait AudioEncoder { type Options:Default ; id() ; extension() ; mime() ; encode(&PcmBuffer,&EncodeMeta,&Options)->Result<Vec<u8>,EncodeError> }
  // ng-score
  trait ScoreExporter { type Options:Default ; id() ; extension() ; mime() ; export(&Project,&Options)->Result<ExportOutput /*bytes+warnings*/,ExportError> }
  trait ScoreImporter { import(&[u8])->Result<ImportOutput /*Project+warnings*/,ImportError> }
  fn validate(&Project)->Report ; fn migrate(&[u8])->Result<Project,ScoreError> ; fn to_canonical_json(&Project)->String
  // ng-timeline
  struct TempoMap ; impl { tick_to_sample(tick,sr)->u64 ; sample_to_tick(sample,sr)->Tick /*floor, for display*/ }
  fn compile(&Project, sr)->Result<Schedule,CompileError>        // rejects projects with validation errors
  impl Schedule { to_bytes()->Vec<u8> ; from_bytes(&[u8])->Result<Self,ScheduleError> }
  // ng-render
  struct Renderer<S: Synth> ; impl {
    new(synth, master:MasterBus, cfg:RenderConfig) ; set_schedule(Schedule) ; set_mix(&[LaneMix]) ; set_loop(Option<(u64,u64)>)
    play() ; pause() ; stop() ; seek(sample:u64) ; audition_on(kind,program,key,vel) ; audition_off(key)
    render_block(l:&mut [f32], r:&mut [f32]) -> BlockStatus{position_sample, position_tick, playing, ended} }
  fn render_offline<S: Synth>(Schedule, S, &OfflineOptions, progress:&mut dyn FnMut(f32)->bool /*false = cancel*/)->Result<PcmBuffer,RenderError>
END
RULE C2.1 note events are (start_sample, end_sample, slot, pitch, vel) ; Renderer issues note_on at start and note_off at (output_clock_at_on + (end - start)) from a fixed-capacity queue (default 512)
          => schedule swap, loop wrap and seek never alter the end of already-sounding notes
RULE C2.2 at equal sample: note_off before note_on
RULE C2.3 blocks split into sub-blocks at event boundaries (note start, queued off, loop end) => sample-accurate
RULE C2.4 pause/stop -> release_all ; seek -> silence_all (short fade) then move ; notes straddling a seek point are NOT revived (known limitation L-01)
RULE C2.5 master chain = dry + Reverb(send bus) -> SoftLimiter ; effects are pure ng-dsp parts, no unseeded randomness
RULE C2.6 set_schedule preserves the current TICK position (old sample->tick, new tick->sample with the new TempoMap)
RULE C2.7 render_offline: render past schedule end until active_voices == 0 AND master energy < -80 dBFS (cap tail_max_sec); loop.mode = tail_fold folds every sample p >= E to S + ((p - S) mod (E - S)), adds it, then truncates to E (file length == loop end)
RULE C2.8 slot = lane index in the lanes array ; reordering lanes changes slot mapping -> schedule must be recompiled

## C3 SCHEDULE BINARY  (little-endian, 8-byte aligned)
LAYOUT Header 32 B : magic "NGSC"(4) version u16=1 flags u16=0 sample_rate u32 note_count u32 lane_count u32 tempo_count u32 total_samples u64
LAYOUT Lane  8 B   : slot u16 program u8 kind u8(0 melodic,1 drum) volume u8 pan u8 reverb_send u8 flags u8(bit0 mute, bit1 solo)
LAYOUT Tempo 16 B  : tick u32 mpq u32 sample u64 (segment start sample)
LAYOUT Note  24 B  : start u64 end u64 slot u16 pitch u8 vel u8 reserved u32=0
ORDER  notes sorted (start, slot, pitch) ; total_samples = max note end (tail excluded)
RULE C3.1 the tempo table lets the worklet convert sample->tick itself ; the UI never contains time-conversion logic

## C4 WASM C ABI  (ng-wasm, no wasm-bindgen)
CONVENTION every function returns i32 (0 OK, negative = error) ; variable-size output goes to the RESULT buffer (ng_result_ptr/len), auxiliary output (warnings) to the AUX buffer (ng_aux_ptr/len) ;
           both valid only until the next ABI call (caller copies at once) ; error detail = ng_error_ptr/len UTF-8 JSON {code,message,path?} ; sample positions are f64 ; pointers are u32
EXPORT ng_alloc(len) | ng_free(ptr,len) | ng_abi_version()->u32
EXPORT ng_project_validate(ptr,len)->RESULT=Report JSON | ng_project_migrate(ptr,len)->RESULT=canonical JSON | ng_project_canonical(ptr,len)->RESULT=canonical JSON
EXPORT ng_compile(ptr,len,sr)->RESULT=Schedule bytes | ng_gm_tables()->RESULT=names/families/drum keys JSON
EXPORT ng_bank_parse(ptr,len)->RESULT=SoundbankImage | ng_bank_load_image(ptr,len) [stopped state only else E_BUSY] | ng_bank_presets()->RESULT=[(bank,program,name)] JSON
EXPORT ng_engine_init(sr, synth_kind /*0 osc,1 soundfont*/, max_voices) | ng_engine_set_schedule(ptr,len) | ng_engine_set_mix(ptr,len)
EXPORT ng_engine_transport(cmd /*0 play,1 pause,2 stop,3 seek*/, arg_f64) | ng_engine_set_loop(start_f64,end_f64,enabled) | ng_engine_audition(kind,program,key,vel,on)
EXPORT ng_engine_render(l_ptr,r_ptr,frames)->status bits(1 playing,2 ended)  [R-RT applies] | ng_engine_position_sample()->f64 | ng_engine_position_tick()->u32
EXPORT ng_offline_begin(proj_ptr,len,opts_ptr,len) | ng_offline_step(max_frames)->frames_done(0 = finished) | ng_offline_progress()->f64 | ng_offline_cancel() | ng_offline_finish()->RESULT=PCM blob
EXPORT ng_encode_wav(pcm_ptr,len,opts_ptr,len)->RESULT=WAV bytes | ng_export_midi(proj_ptr,len,opts_ptr,len)->RESULT=SMF, AUX=warnings | ng_import_midi(ptr,len)->RESULT=project JSON, AUX=warnings
BLOB PCM 16 B header: magic "NGPC" sample_rate u32 frames u32 channels u16 reserved u16 ; then f32 LE interleaved
JSON offline_options {"sample_rate":44100,"tail_max_sec":10,"loop":{"mode":"none|tail_fold"},"solo_lane":null,"ignore_mute_solo":false,"synth":"auto|osc|sf"}
RULE C4.1 ng_abi_version mismatch -> TS refuses to load
RULE C4.2 every function tolerates invalid pointers/lengths by returning an error code (no trap)

## C5 WORKLET PROTOCOL
MSG main->worklet  construct: processorOptions{module:WebAssembly.Module, sampleRate, synthKind}  (fallback: send bytes and compile inside the worklet [VERIFY: spike:SP-1])
MSG main->worklet  {t:'bank', image:ArrayBuffer(transfer)}  stopped state only
MSG main->worklet  {t:'schedule', bytes(transfer)} any time | {t:'mix', bytes} | {t:'transport', cmd, arg} | {t:'loop', start, end, enabled} | {t:'audition', ...}
MSG worklet->main  {t:'position', sample, tick, playing} ~30 Hz | {t:'ended'} | {t:'bank_ready', presets} | {t:'error', code, message}

## C6 TYPESCRIPT INTERFACES
API
  interface AudioEngine { init():Promise<void> /*user gesture*/ ; loadSoundbank(image:ArrayBuffer):Promise<PresetInfo[]> /*stopped only*/ ; setSchedule(bytes) ; setMix(lanes:LaneMix[])
                          play() ; pause() ; stop() ; seek(tick) ; setLoop(range|null) ; audition(kind,program,key,vel,on) ; onPosition(cb):()=>void ; dispose() }
  interface ProjectStore { list():Promise<ProjectMeta[]> ; load(id):Promise<Uint8Array> ; save(id, bytes, opts?:{autosave?}) ; remove(id) }
  interface SoundbankSource { id:'url'|'local'|string ; label ; isAvailable():boolean ; acquire(req:BankRequest, onProgress:(loaded,total?)=>void, signal?):Promise<{name, bytes:Uint8Array}> }
  interface OutputSink { id:'download'|'directory'|string ; label ; isAvailable():boolean ; write(name, bytes, opts:{overwrite:'ask'|'replace'|'rename'}, signal?):Promise<{finalName}> }
  interface AudioEncoder { id:'wav'|'ogg'|string ; extension ; mime ; encode(pcm:PcmBlob, meta:EncodeMeta, opts?, signal?):Promise<Uint8Array> }
  interface Command { label ; apply(doc:Document):Document /*pure, structural sharing*/ }
  interface History { execute(c) ; undo() ; redo() ; canUndo ; canRedo }
  type Intent = addNote{lane,start,pitch,duration?} | moveNotes{ids,dTick,dPitch,toLane?} | resizeNotes{ids,edge:'start'|'end',dTicks} | deleteNotes{ids} | setVelocity{ids,vel}
              | addLane{kind,program} | setLaneProgram{lane,program} | setTempo{tick,bpm} | setLoop{range|null} | paste | duplicate | quantize | transpose
END
RULE C6.1 a Command never yields a document violating I-01..I-10 (overlaps auto-trimmed; lane removal handles its notes)
RULE C6.2 in test/dev mode every Command is followed by ng_project_validate

## C7 REALTIME AND DETERMINISM
RULE R-RT-1 zero heap allocation in render_block path after init (schedule swap happens in the message handler, outside process)   CHECK gate:g15
RULE R-RT-2 no panic: panic=abort ; HOTPATH denies unwrap/expect/indexing/panic                                                     CHECK gate:g01
RULE R-RT-3 denormal protection in feedback structures (reverb, filters)                                                            CHECK gate:g15
RULE R-RT-4 determinism: libm for transcendentals, no HashMap order, no clock, noise from seeded LCG ; native CLI render == wasm render bit-for-bit  CHECK gate:g05
RULE R-RT-5 bank load/parse never on the audio thread

## C8 EXPORT FORMATS
FORMAT wav   RIFF/WAVE ; chunks fmt, [smpl], [LIST/INFO INAM], data ; 16/24-bit int PCM or 32-bit float ; int convert = round(x * (2^(n-1)-1)) then clamp ; dither OFF by default (seeded TPDF optional) ;
             clip count reported ; smpl = one infinite loop only when loop given ; chunks padded to even length ; > 4 GiB -> E_ENCODE
FORMAT midi  SMF format 1, division 480 ; track 0 = conductor (tempo FF51, meter FF58, title FF03) ; one track per lane (name FF03, [port FF21], bank CC0/CC32, program change, CC7, CC10, CC91, notes)
             same-tick order: note-off, controllers, note-on ; no running status ; muted/solo lanes follow the audio rule unless include_muted=true
             channel plan (deterministic): lanes in order take the next free channel of the current port ; melodic skips channel index 9 ; drum lane = channel 10 of a port ; no room -> next port + W_MIDI_MULTI_PORT
FORMAT ogg   libvorbis VBR ; quality -0.1..1.0 (default 0.4) ; input 44100/48000 Hz stereo ; file length == loop end when looped (game needs the loop START only)
             comments TITLE, [LOOPSTART, LOOPLENGTH in samples] ; sidecar <name>.loop.json = {sample_rate, loop_start_sample, loop_end_sample, total_samples, loop_offset_sec}
RULE C8.1 Godot 4.8-dev6 usage after copying music_output -> core/assets/audio/music: import dock Loop ON ; Loop Offset = loop_offset_sec when the loop start != 0 ; runtime alternative: read sidecar, set AudioStreamOggVorbis.loop / loop_offset [VERIFY]
RULE C8.2 release notes seen for 4.8 dev6/dev7 list no OGG/WAV import changes, full changelog not checked [VERIFY] ; recheck on the 4.8 stable release

## C9 ERROR CODES
ERROR E_FORMAT E_VERSION_TOO_NEW E_UNKNOWN_FIELD E_RANGE E_LANE_DUP E_LANE_MISSING E_OVERLAP E_TEMPO E_METER E_LOOP
ERROR E_BUSY E_BANK_FORMAT E_BANK_UNSUPPORTED E_BANK_FETCH E_RENDER_CANCELED E_ENCODE E_MIDI_PORTS E_STORAGE E_PATH E_EXISTS E_SINK_PERMISSION
WARN  W_NOTE_PAST_LENGTH W_DRUM_KEY_RANGE W_UNUSED_LANE W_BPM_ROUNDED W_MIDI_MULTI_PORT W_FALLBACK_SYNTH W_IMPORT_TRIMMED
SHAPE Report {errors:[{code,message,path}], warnings:[...]} ; path = JSON Pointer ; collect ALL issues, never stop at the first

## C10 FILE CONTRACT OF BINARY IMAGES
RULE C10.1 SoundbankImage "NGSB": versioned, little-endian, aligned sample blob ; same SF2 -> byte-identical image ; from_image validates version and ranges ; magic check of source = "RIFF" .. "sfbk"
RULE C10.2 source limits: <= 256 MB (warn > 64 MB) ; recommended image <= 64 MB (each engine thread holds a copy)

## C11 BROWSER CONTRACT  (desktop Chrome, Edge, Firefox, Safari; latest 2)
EXPECTED  [VERIFY in spike:SP-1; table is design-time expectation]
  AudioWorklet+WASM   required ; absent -> start screen explains, editing only
  IndexedDB           all ; Safari may purge script-writable storage after long inactivity [VERIFY] -> see T04
  File System Access  Chromium only ; fallback = <input type=file> + download
  Service worker      all (some private modes limited) ; absent -> online-only warning
  WASM SIMD / OffscreenCanvas  expected on current versions ; fallback = non-SIMD build / main-thread canvas
RULE C11.1 feature detection only
RULE C11.2 no SharedArrayBuffer, no cross-origin isolation
RULE C11.3 WASM offline render output is byte-identical across engines                      CHECK gate:g14
RULE C11.4 AudioContext sampleRate is device-dependent: compile with the context rate ; export rate is independent

## C12 OUTPUT WRITE SAFETY  (browser sinks and CLI FsSink implement the same rules; shared test vectors)
RULE C12.1 names: title -> slug [a-z0-9_-], lower-case, <= 64 chars, empty -> "untitled" ; extension decided by format (.wav .ogg .mid .loop.json .ngproj.json) ;
           reject (E_PATH): path separators, "..", absolute paths, control chars, Windows reserved names {CON,PRN,AUX,NUL,COM1-9,LPT1-9}, trailing dot/space
RULE C12.2 write only directly inside the sink folder (no sub-folders) ; CLI folders = ROOT/music_output and ROOT/music_source resolved by the nearest ancestor holding notegrid.root ;
           compare realpaths (symlinks/junctions resolved, case-insensitive on Windows) ; outside -> E_PATH ; no escape flag
RULE C12.3 existing name: UI asks (replace | save as _2 | cancel) ; CLI refuses with E_EXISTS unless --force
RULE C12.4 atomic: CLI writes .tmp then renames ; DirectorySink uses createWritable() commit-on-close and abort() on cancel/error ; never leave partial files
RULE C12.5 the tool never deletes or moves any file (no prune)
RULE C12.6 sinks: DownloadSink (all browsers; UI states files go to the browser download folder) ; DirectorySink (Chromium directory picker; only the chosen handle is stored; permission re-requested) ; denied -> E_SINK_PERMISSION
RULE C12.7 reflecting outputs into the game folder is not the tool's job (DR-3); an optional sync script lives OUTSIDE ROOT
