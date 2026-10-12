# i2 feature inventory (static+docs route — runtime RE blocked, see bottom)

## Installer identity (observed, shipped artifact)
- `mi2_std_1.1.6.0905-x64.exe` (31,637,088 B, sha256 8cc61e29...) = InstallShield Unicode bootstrapper (ISSetup.dll/msiexec strings, "MoTeC i2 Standard (x64) Installer", (c) MoTeC 2003-2024).
- PE32 x86, 5 sections (~1.1MB); ~30.5MB appended payload at 0x110400 magic `ISSetupStream` + compressed/encrypted blob (entropy 8.0, no ZIP/RAR/MZ/OLE/CAB signatures).
- NOT the i2 app itself; it wraps `MoTeC i2 Standard (x64) 1.1.msi` resolved at install time.
- REA portable ops: inspect-artifact (single-node PE), inspect-pe-resources (69 resources, icons), inspect-managed-artifact (native, no CLI).

## Feature list (from MoTeC docs/APIs, with sources)
Product matrix https://www.motec.com.au/i2/i2overview (Std vs Pro where noted):
- DATA: open .ld, telemetry streams (t2://), overlays (Std 1 / Pro unlimited), graphical overlay alignment, data gating, cursor+zoom linking, animation, edit beacons/lap times, lap stretching, data export (graph/histogram/reports; CSV/MAT/LD via API EDataExtent).
- GRAPHS: time/distance graphs, multiple overlays, templates (30, F5/F7), channel editor (scale/color), filter/scale-offset/zero-at-cursor (stored as local maths) — source: i2 Training Seminar 2008 + API guide.
- TRACK: auto track generation (speed+latG+beacon), track/rainbow/lap maps, section editor + user sections, section-time reports + eclectic lap.
- STATS/REPORTS: histograms (Std 2ch/Pro unlimited; susp-velocity, multi-ch), scatter/XY plots + regression (Std 2x2/Pro unlimited), mixture maps, channel reports (min/max/avg/stdev per section, reverse axis), acceleration reports, lap reports, statistics + engine-log diagnostics.
- MATHS: equation editor (Ctrl+M, folders, units, help), predefined fns (smooth/scale/offset/filter/distance/corrected speed/corrected latG std; rest incl integrate/derivative/choose/gating = Pro), regression channels, user constants, setup-sheet constants in maths, comments on maths/constants, integer enumerations incl DBC VAL_TABLE import, maths import/export (.xml), maths plug-ins (IMtcMathPlugin), on-demand calc.
- DETAILS/SETUP: details editor (numeric/string/datetime + units + DPS), compare side-by-side, Excel setup sheets, vehicle constants, device configuration view.
- PROJECTS/WORKSPACE: user projects (vehicle/race-type profiles), worksheet layouts, color schemes, global channel colors/scales/units, channel aliases+mapping, drag-drop channel select, reference laps, app profiles (Circuit/Drag/Bike/Rally).
- API (COM, TypeLib v2.3/2.4, timebase µs): Application/Single, DataSources Open(t2:// or .ld)/Create/ExportMain(LD|MAT|CSV), Layers (Normal/Burst0/Burst1/Details/Fastest/SetupSheet), Channels+Descriptors (color/DPS/interp/scale/unit), DataArray (samples/flags/resample aaNone/Avg/Min/Max), Markers (BCN/SPLTBCN/IGRDBCN/RESET/SOL/EOL) + RangeGroups (Outings:Laps/Splits, Runs, Stages, Data), Details, SetupSheet, Qtys/Units SI convert, Query API.
- Lineage (Interpreter manual 1998-2002, still recognizable): burst/fastest-lap logging modes, virtual dash replay, mixture map, statistics, graph hotkeys (F-keys above).
- V1.1.4 deltas (local Feature Guide): DBC VAL_TABLE enums, telemetry save-on-exit, log-open shortcuts, channel-report reverse axis, gauge alarms.

## What runtime RE would still need a Windows host for (deferred)
- Actual i2 UI strings/dialogs/behavior, COM ProgID tracing (MoTeC.i2Application), sample workspace+log bundle inside the MSI, telemetry protocol bytes. Path: install on Windows + process/API capture per skill runtime-observation guide.
