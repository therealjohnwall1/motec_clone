# MoTeC .ld / .ldx format (synthesis)

Sources:
- https://pages.tobi.lutke.com/racingmagick/motec_format (reverse-engineered, verified Oreca 07 LMP2; full offsets)
- https://github.com/afonso360/motec-i2 (Rust read+write, MIT; src/reader.rs, structs.rs)
- https://github.com/gotzl/ldparser (Python, 80 stars, tested vs Mi2 Pro sample logs; ACC telemetry)
- https://github.com/riccardotornesello/motecldparser (Go read/write)
- https://github.com/Melbourne-University-Motorsports/motec-to-csv (Int16/32/Float + resampling)
- https://github.com/TegoSys/MotecFile (naive header-scan parser; NOT authoritative)
- https://github.com/stevendaniluk/MotecLogGenerator (CAN+DBC->LD Pro Enabled writer)

## .ld header (little-endian, Latin-1 strings)
- 0x00 u32 magic = 0x40 (64). Reject otherwise.
- 0x08 u32 channel_meta_ptr, 0x0C u32 channel_data_ptr, 0x24 u32 event_ptr (0=none)
- 0x5E[16] date dd/MM/yyyy, 0x7E[16] time HH:mm:ss, 0x9E[64] driver, 0xDE[64] vehicle_id, 0x15E[64] venue
- Extended (gotzl ldHead fmt / afonso reader.rs): device_serial u32, device_type[8] (e.g. ADL), device_version u16, num_channels u32, session[64], short_comment[64], pro-logging magic u32 (0xc81a4 enables Pro), long_comment area.
- Event chain: event_ptr -> Event(name[64], session[64], comment[1024], venue_addr u16) -> Venue(name[64], +1034 pad, vehicle_addr u16) -> Vehicle(id[64], +128 pad, weight u32, type[32], comment[32]).

## Channel metadata: 124B linked list from channel_meta_ptr (prev/next==0 ends; cap ~200; clamp to file size)
- +0x00 prev u32, +0x04 next u32, +0x08 data_ptr u32, +0x0C n_data u32
- +0x10 id u16 (unused), +0x12 datatype_a u16, +0x14 datatype(size:2|4) u16, +0x16 rec_freq Hz u16
- +0x18 shift i16, +0x1A mul i16, +0x1C scale i16, +0x1E dec_places i16
- +0x20 name[32], +0x40 unit[12], +0x4C short[8], then display/range floats + reserved to 124B
- datatype_a: 7=float (no conversion), 3/5/0=signed int (convert); size 2=i16/f16, 4=i32/f32; 8/8=float64 (gotzl ACC); Beacon16/32 behave as int; Invalid (0,5),(15,5),(17536,5)... = 0-sample iRacing-mu artifacts, skip.

## Conversion (integer -> physical) — all three sources agree
scale_eff = scale==0 ? 1 : scale; mul_eff = mul==0 ? 1 : mul
value = (raw / scale_eff * 10^(-dec_places) + shift) * mul_eff   // +offset asserted 0 in afonso
float32/float64: value as-is. Reader must clamp data_ptr+n*size to file size; n_data==0 valid empty.

## .ldx companion (UTF-8 XML, same basename)
- Pattern `<Marker[^>]* Time="(\d+)"`, Time in MICROseconds -> seconds = /1e6. Sort beacons; laps = consecutive pairs; 0/1 beacons or missing file -> single lap [0, max_duration].

## Resampling / units (racingmagick + Melbourne)
- Freqs 10..200Hz; unify (e.g. 50Hz) via linear interp; step-interp for enum/gear/beacon types.
- Speed->km/h (m/s*3.6, mph*1.60934), brake->bar (psi*0.0689476, kpa*0.01), throttle ratio/%/deg/rad ->0..1, steering rad->deg.
- Names case-insensitive: Corr/Ground/Wheel/Aero Speed, Driver/Throttle Pos, P_F_BRAKE/Brake Pos, Steering Angle, Gear, G Force Long, X_FL/FR/RL/RR_DAMPER, Engine RPM.
