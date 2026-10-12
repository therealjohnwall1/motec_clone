# REA-driven reverse loop (per major feature branch)

State: portable REA done; deep native RE blocked until Ghidra on supported host or Windows+i2 runtime.
Per-feature loop (matches requested branch->build->user GUI review):
1. REA question pass (reuse Evidence IDs above): literal search targets (e.g. beacon VAL_TABLE strings, pro-logging magic 0xc81a4, COM ProgID) + artifact resources; record Observations vs Inferences vs Unknowns.
2. If static insufficient AND runtime available: bounded capture (process/API/COM or file-watch on .ld save) with explicit target+duration; never broaden scope.
3. Implement in Rust behind feature flag; golden tests vs Sample1.ld + ACC/MotecLogGenerator samples + your real logs (to be supplied).
4. Export Evidence bundle (`export_evidence_bundle`) per branch into motec_rev/agents/evidence/<branch>.json.
5. GUI checkpoint: user runs cross-platform binary, reviews plots/zoom/scroll/messages/DBC, reports feedback; next branch only after sign-off.
Branch order proposal (to confirm in plan): B0 scaffold+LD read+channel list -> B1 LDX laps+time axis -> B2 plot+zoom/scroll -> B3 units/maths/details+custom messages -> B4 DBC import(+VAL_TABLE)+CAN decode -> B5 library/store+export(CSV/MAT/LD) -> B6 telemetry-stub/workspace parity.
Windows-only behaviors (COM API, Excel SetupSheet, MATLAB export) get Rust-native equivalents; no Wine dependency in main binary.
