# Existing parser comparison (what to reuse vs rewrite in Rust)

1. afonso360/motec-i2 (Rust, MIT, 24 stars) — BEST BASE
   - Cargo: edition 2021, dep byteorder only. Examples read/write. Sample Sample1.ld (78 ch, Calder 23/11/2005).
   - reader.rs: full header/event/venue/vehicle/channel/data iterators; tests assert exact header+channels+data (Air Temp Inlet 19.9C etc).
   - structs.rs Datatype::from_type_and_size covers (0,2/4)Beacon,(3+5,2/4)int,(7,2/4)float + Invalid for mu-exporter artifacts.
   - writer.rs + full_header.rs (13384B template): writes Pro-enabled files. LIMITS: F16 unimplemented, offset asserted 0, ldx module present but verify beacon write path, no memmap/lazy, no DBC.
   - Plan: vendor as `motec-ld` crate core (dependency or fork), add: memmap2 lazy channel handles, F16 (half crate), beacon/enum channels, ldx quick-xml read/write, property tests vs Sample1.ld + ACC logs.

2. gotzl/ldparser (Python GPL-3.0, 80 stars)
   - Struct fmts are ground truth cross-check: ldHead/ldEvent/ldVenue/ldVehicle/ldChan; dtype map incl (8,8)->float64; write path with conv_data inverse ((data/mul - shift)*scale/10^-dec).
   - Tested vs Mi2 Pro bundled samples + ACC. frompd() shows DataFrame->LD path (fixed 10Hz float32) — informs our CSV/CAN import.
   - License GPL blocks direct copy into MIT Rust; use as BEHAVIORAL ORACLE only.

3. stevendaniluk/MotecLogGenerator (Python, CAN+DBC->LD Pro Enabled)
   - Inputs: candump -l logs + DBC (signal name+units -> MoTeC channel), CSV (col0=time), COBB Accessport CSV; resamples to fixed freq (MoTeC demands constant rate).
   - can_utils/: candump_converter, dbc_file_from_can_log, list_can_ids/messages; examples: can_sample.log (3.6M), sample_can_spec.dbc.
   - Plan: mirror pipeline in Rust: `can-dbc`/`dbc-codegen` crate for DBC VAL_TABLE enums + message decode -> resample (linear/step by signal type) -> LD writer. Keep Python as oracle for byte-compare tests.

4. riccardotornesello/motecldparser (Go) + Melbourne motec-to-csv (Python)
   - Go: generics Channel[T] float32/int16/int32, File{Time,Driver,Vehicle,Venue,ShortComment}+AddChannels+Write; HEADER/EVENT/VENUE/VEHICLE/CHHDR/DATA layout doc.
   - Melbourne: Int16/32/Float + intelligent linear/step resample + time-align — adopt their interp-type rule.

5. TegoSys/MotecFile — naive (marker scan 01 00 01 00 00 00, fixed offset 25186, all-int16). WRONG for mixed types; do not use except as negative test.
