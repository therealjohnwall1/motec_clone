# REA skill status (2026-10-12, Debian 13 x86_64)

Skill: reverse-engineer-anything v37 (`~/.agents/skills/reverse-engineer-anything/SKILL.md` + `references/native-and-artifacts.md`).
REA CLI: `npx -y rea-agents@6.4.0` works.

`rea doctor` result:
- node 22.23.2 healthy
- host UNSUPPORTED (Debian 13; supports macOS 12+, Ubuntu 24.04+, Fedora 41+, Nobara 44+, Arch/CachyOS/Omarchy, experimental Win x64 Ghidra P0)
- ghidra MISSING (GHIDRA_INSTALL_DIR unset), hopper MISSING, ida NOT configured
- skill/registrations stale (opencode registration drift)

Portable ops VERIFIED working on this host (with Evidence IDs retained):
- `inspect-pe-resources ./i2_stuff/mi2_std_1.1.6.0905-x64.exe` -> ev_14a50e6b68235defab593dc2d6baba0875728d3647e0114c5d9d8dc45da8e690 (PE32, machine 0x14c=x86, 69 resources, icons verified)
- `inspect-artifact <exe>` -> manifest agm_0fb11f..., single-node PE, no child container (installer is NOT zip; needs execution or InstallShield unpack, not static unzip)
- `inspect-managed-artifact <exe>` -> ev_9a763d20d4cbc8b590b7e24c70e924e81a8f0ec315511fba4cc871d7fa1b5927 : NOT managed (no CLI dir), native x86 PE32, 5 sections

Installer findings:
- `file` says PE32 i386 GUI 5 sections for both mi2_pro/mi2_std despite x64 names (likely 32-bit stub/bootstrapper)
- byte scan: contains `InstallShield` marker at ~513936, `Qt` at ~28068; no NSIS/Inno/7-Zip/WiX/.NET/MFC strings in first 4MB
- Implication: deep RE (open_binary/analyze_function/decompile) requires Ghidra on supported host OR Windows VM with i2 installed + runtime observation (process capture, API/COM tracing). Per skill: do NOT repeat setup per call; use portable evidence until engine available.

REA loop we will use per feature (skill §Plan broader investigations):
1. question -> evidence needed; 2. reuse session/evidence; 3. smallest overview first; 4. resources+config alongside code; 5. corroborate (static vs runtime only if needed); 6. parallel workers per independent question; 7. ledger with Evidence IDs + confidence + unknowns.
Native route once engine available: open_binary -> binary_overview -> literal search (e.g. "VAL_TABLE", ".ld", "Beacon", COM progid MoTeC.i2Application) -> analyze_function pseudocode -> value_flow facet via inspect_analysis_view.
