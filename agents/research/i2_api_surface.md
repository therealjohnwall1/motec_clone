# i2 feature surface (from your i2_stuff PDFs)

A. i2_API_User_Guide (V1.1.4 API, COM, Type Library v2.3/2.4):
   - Licence-gated `i2.Application` / `ApplicationSingle` (ProgID MoTeC.i2Application). Time base MICROseconds.
   - Object model: DataSources -> DataSource{Date,Time,Details,Layers,Quantities,Units,SetupSheet} -> DataLayers{Normal/Burst0/Burst1/Details/Fastest/SetupSheet} -> Channels{IMtcChannel: DataArray+Descriptor+Status} -> DataArray{Samples,Rate,Start/EndTime,SampleFlags,SampleIndex/Time,GetValue(interp-aware)}.
   - ChannelDescriptor: Id/Type(MoTeC.Integer/Float/String)/Unit/Color/DPS/Interpolate/ScaleMin/Max. Channel2/3 add resample: DataArraySegmentFromIndexes/Interval + AntiAliasFilterType(aaNone/Avg/Min/Max).
   - Markers: BCN/SPLTBCN/IGRDBCN/RESET/SOL/EOL, MarkerGroups, AddTimeMarker/AddDistMarker(parent,class,name,dist/time,comment).
   - Ranges: Outings:Laps (+Splits) for Circuit, Runs (Drag), Stages (Rally), Data (Engine); Range{Abbrev,Enabled,Trusted,Start/End markers}; SupportsReporting/TrackGeneration.
   - Details: AddDateTime/AddNumeric(AddNumeric Id,Value,Unit,DPS)/AddString; SetupSheet (Excel) details.
   - IO: Open(t2:// telemetry or .ld path)/Close/Create(Duration)/ExportMain(LD)/ExportMainAsMAT(MATLAB5)/ExportMainAsCSV + EDataExtent(deMainRange/MainRangeOuting/MainRangeZoom) + ESampleRateOpt(srCustom/Default/Fastest).
   - Math: IMtcMathFunction(Evaluate), IMtcMathPlugin(InputChannels/OutputChannels/Settings/Summary, Execute/Register/UnRegister/ShowSettings); Math API is how "custom messages"/derived channels appear.
   - Units/Qtys: FromSI/ToSI per unit; full list in Tools|View Units.

B. i2 Feature Guide V1.1.4 deltas:
   - DBC import: ONLY VAL_TABLE enum entries imported into i2 (plan enum-channel support accordingly).
   - Telemetry save-on-exit: car-number + source name -> LD export (plan live t2:// stub + file export parity).
   - Log File Open shortcuts (deep folder trees) — informs our library UX.
   - Reports incl Channel Report (Reverse main axis), Acceleration/Chart/Text/Split views — informs our report/plot parity scope.

C. Interpreter Manual (M1 Launch Control PDF) — TODO: extract custom-message/math expression grammar; likely source of "custom messages" semantics. Pending read (PDF parse via Read tool next).
