# motec_rev — cross-platform i2 in Rust (egui+wgpu), Linux-first

Goal: read/store/select LD+LDX in a chosen directory and show ALL contained information like i2 on Windows: plots, zoom/scroll time domain, custom messages, DBC decode, laps/beacons, track/XY, histograms/reports, details/metadata, exports.

## Decisions (user-confirmed 2026-10-12)
- Core: fork MIT `afonso360/motec-i2` (read/write LD+LDX), `gotzl/ldparser` GPL as behavior oracle only.
- GUI: pure Rust `egui + wgpu` single binary; software fallback (wgsl→gl/low-power) for no-GPU/Pi.
- DBC: full CAN decode + VAL_TABLE enums (i2 imports VAL_TABLE only — Feature Guide p.9 — but we decode full frames into channels like MotecLogGenerator).
- Scope v1: everything the file contains (all header/event/venue/vehicle/channel fields) + all views (time plots, laps/outings, track/XY, histograms/reports, maths/derived).
- REA: portable-only for now (Debian 13 unsupported host, no Ghidra/Hopper). Evidence retained in `agents/research/REA_skill_status.md`.
- Workflow: branch per major feature, commit as we go, call user for GUI review each branch.

## How conversion works (do not re-derive; tested)
- magic u32 0x40; header ptrs (meta/data/event); Latin-1 strings; event->venue->vehicle chain.
- channels: 124B linked list; datatype_a 7=float as-is; else int via `(raw/scale_eff * 10^-dec + shift) * mul_eff` (scale/mul 0→1); sizes 2=i16,4=i32 (+f16 rare, f64 8/8 for ACC).
- LDX: `<Marker Time="µs">` → laps = consecutive beacon pairs; missing/≤1 beacon → single lap.
- Resample mixed 10–200Hz to common rate (linear; step for gear/beacon/enum); unit normalize (m/s→km/h etc.).
- Full spec: `agents/research/ld_ldx_format.md`. Parser survey: `agents/research/existing_parsers.md`. i2 object model (layers/markers/ranges/details/math/COM timebase µs): `agents/research/i2_api_surface.md`.

## Branch plan (each = branch → build → user GUI review → feedback)
- B0 scaffold: cargo workspace (`motec-ld` fork core + `motec-gui` egui), dir library (watch chosen dir for .ld/.ldx), channel list + all-fields inspector, golden tests vs Sample1.ld. REA: reuse portable evidence only.
- B1 laps/time: LDX beacons, outing/lap tree, time axis, cursor readouts, details/event/venue/vehicle panels.
- B2 plots: multi-channel time/distance plots, zoom/scroll/pan, cursors, decimation for large logs, reference lap overlay.
- B3 maths/messages: derived channels (expression eval), custom messages/details, units/DPS/interp, VAL_TABLE enums display.
- B4 DBC/CAN: DBC import + candump decode → channels/LD (mirror MotecLogGenerator pipeline in Rust), resample fixed-freq.
- B5 store/export: library persistence, CSV/MAT/LD export (EDataExtent main/outing/zoom parity), shortcuts for deep folders.
- B6 parity: track map + XY, histograms, channel report (incl reverse axis), telemetry-stub import. Windows/mac packaging after Linux green.

## REA loop per branch (skill v37 §Plan broader investigations)
1. Question→evidence map; reuse prior Evidence IDs; smallest overview first; resources+config with code; static-first, runtime only if blocked+available; ledger (observed/inferred/unknown) + `agents/evidence/<branch>.json`.

## Next: user drops real LD/LDX+DBC+custom-message examples (tell me where); I start B0 scaffold.
