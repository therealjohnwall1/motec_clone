use egui_plot::{Legend, Line, Plot, PlotPoints, VLine};
use motec_ld::{ChannelMetadata, Event, Header, LDReader, MathDef, MathInput, eval_math};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

const PALETTE: [egui::Color32; 8] = [
    egui::Color32::from_rgb(86, 180, 233),
    egui::Color32::from_rgb(230, 159, 0),
    egui::Color32::from_rgb(0, 158, 115),
    egui::Color32::from_rgb(204, 121, 167),
    egui::Color32::from_rgb(213, 94, 0),
    egui::Color32::from_rgb(0, 114, 178),
    egui::Color32::from_rgb(240, 228, 66),
    egui::Color32::from_rgb(204, 51, 51),
];

struct LoadedLog {
    path: PathBuf,
    header: Header,
    event: Option<Event>,
    channels: Vec<ChannelMetadata>,
    /// Lap beacon times in seconds (from .ldx, or empty = whole session).
    beacons: Vec<f64>,
}

fn load_log(path: &PathBuf) -> Result<LoadedLog, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut src = BufReader::new(f);
    let mut r = LDReader::new(&mut src);
    let header = r.read_header().map_err(|e| format!("header: {e}"))?;
    let event = r.read_event().map_err(|e| format!("event: {e}"))?;
    let channels = r.read_channels().map_err(|e| format!("channels: {e}"))?;
    let beacons = load_beacons(path);
    Ok(LoadedLog { path: path.clone(), header, event, channels, beacons })
}

/// Parse `<Marker Time="µs">` entries from the companion .ldx file.
/// Returns seconds; empty vec when no .ldx or no beacons (single-lap fallback).
fn load_beacons(ld_path: &Path) -> Vec<f64> {
    let mut p = ld_path.to_path_buf();
    p.set_extension("ldx");
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut out = vec![];
    let mut rest = text.as_str();
    while let Some(i) = rest.find("Time=\"") {
        rest = &rest[i + 6..];
        if let Some(end) = rest.find('"') {
            if let Ok(us) = rest[..end].parse::<f64>() {
                out.push(us / 1_000_000.0);
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    out
}

fn decode_all(path: &Path, ch: &ChannelMetadata) -> Result<Vec<f64>, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut src = BufReader::new(f);
    let mut r = LDReader::new(&mut src);
    let d = r.channel_data(ch).map_err(|e| format!("data: {e}"))?;
    Ok(d.iter().map(|s| s.decode_f64(ch)).collect())
}

fn list_ld_files(dir: &str) -> Result<Vec<PathBuf>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("read dir '{}': {e}", dir))?;
    let mut out = vec![];
    for e in rd {
        let e = e.map_err(|e| format!("dir entry: {e}"))?;
        let p = e.path();
        if p.extension().map(|x| x.eq_ignore_ascii_case("ld")).unwrap_or(false) {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

struct App {
    dir: String,
    files: Vec<PathBuf>,
    dir_err: String,
    loaded: Option<LoadedLog>,
    load_err: String,
    filter: String,
    selected: Option<usize>,
    plotted: Vec<usize>,
    cache: HashMap<usize, Vec<f64>>,
    preview: String,
    hover: String,
    // ---- B3 maths ----
    maths: Vec<MathDef>,
    math_data: HashMap<String, (f64, Vec<f64>)>,
    math_plotted: Vec<String>,
    sel_math: Option<String>,
    math_err: String,
    m_name: String,
    m_expr: String,
    m_rate: String,
    m_unit: String,
}

impl App {
    fn new() -> Self {
        let dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| String::from("."));
        let files = list_ld_files(&dir).unwrap_or_default();
        Self { dir, files, dir_err: String::new(), loaded: None, load_err: String::new(),
               filter: String::new(), selected: None, plotted: vec![], cache: HashMap::new(),
               preview: String::new(), hover: String::new(),
               maths: vec![], math_data: HashMap::new(), math_plotted: vec![], sel_math: None,
               math_err: String::new(), m_name: String::new(), m_expr: String::new(),
               m_rate: String::new(), m_unit: String::new() }
    }

    fn refresh_files(&mut self) {
        match list_ld_files(&self.dir) {
            Ok(f) => { self.files = f; self.dir_err.clear(); }
            Err(e) => self.dir_err = e,
        }
    }

    fn open(&mut self, path: PathBuf) {
        match load_log(&path) {
            Ok(l) => {
                self.loaded = Some(l);
                self.load_err.clear();
                self.selected = None;
                self.sel_math = None;
                self.plotted.clear();
                self.cache.clear();
                self.math_data.clear();
                self.math_plotted.clear();
                self.preview.clear();
            }
            Err(e) => self.load_err = e,
        }
    }

    /// Decode (or reuse cache for) the logged channels an expression needs,
    /// then evaluate the math def. Only referenced channels are decoded.
    fn ensure_math(&mut self, def_idx: usize) -> Option<String> {
        let def = self.maths.get(def_idx)?.clone();
        if self.math_data.contains_key(&def.name) {
            return None;
        }
        let refs = match motec_ld::referenced_channels(&def.expr) {
            Ok(r) => r,
            Err(e) => return Some(e.to_string()),
        };
        // resolve names -> (cache pos, canonical name, rate); end borrows before decoding
        enum Need { Known(usize), Missing(String) }
        let needs: Vec<(String, Need)> = {
            let log = match self.loaded.as_ref() {
                Some(l) => l,
                None => return Some("no log loaded".into()),
            };
            refs.iter().map(|r| {
                match log.channels.iter().position(|c| c.name.eq_ignore_ascii_case(r)) {
                    Some(pos) => {
                        let c = &log.channels[pos];
                        (c.name.clone(), Need::Known(pos))
                    }
                    None => (r.clone(), Need::Missing(r.clone())),
                }
            }).collect()
        };
        for (_, need) in &needs {
            match need {
                Need::Known(pos) => {
                    if let Some(e) = self.data(*pos) {
                        return Some(e);
                    }
                }
                Need::Missing(r) => {
                    let avail: Vec<String> = self.loaded.as_ref().unwrap().channels.iter().map(|c| c.name.clone()).collect();
                    return Some(motec_ld::MathError::UnknownChannel { name: r.clone(), available: avail }.to_string());
                }
            }
        }
        let mut inputs: HashMap<String, MathInput> = HashMap::new();
        {
            let log = self.loaded.as_ref().unwrap();
            for (canon, need) in &needs {
                if let Need::Known(pos) = need {
                    let rate = log.channels[*pos].sample_rate.max(1) as f64;
                    inputs.insert(canon.clone(), MathInput { rate_hz: rate, data: &self.cache[pos] });
                }
            }
        }
        match eval_math(&def, &inputs, &Default::default()) {
            Ok((rate, vals)) => { self.math_data.insert(def.name.clone(), (rate, vals)); None }
            Err(e) => Some(e.to_string()),
        }
    }

    fn data(&mut self, idx: usize) -> Option<String> {
        if self.cache.contains_key(&idx) {
            return None;
        }
        let log = self.loaded.as_ref()?;
        let ch = log.channels.get(idx)?;
        match decode_all(&log.path, ch) {
            Ok(v) => { self.cache.insert(idx, v); None }
            Err(e) => Some(e),
        }
    }

    fn add_math(&mut self) {
        self.math_err.clear();
        let name = self.m_name.trim().to_string();
        if name.is_empty() {
            self.math_err = "channel name required".into();
            return;
        }
        if self.maths.iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
            self.math_err = format!("'{name}' already exists");
            return;
        }
        if self.loaded.is_none() {
            self.math_err = "load a log first".into();
            return;
        }
        let rate = if self.m_rate.trim().is_empty() {
            None
        } else {
            match self.m_rate.trim().parse::<f64>() {
                Ok(r) => Some(r),
                Err(_) => {
                    self.math_err = "rate must be a number 1..=1000 (or blank)".into();
                    return;
                }
            }
        };
        self.maths.push(MathDef { name, expr: self.m_expr.clone(), rate_hz: rate, unit: self.m_unit.clone() });
        let di = self.maths.len() - 1;
        if let Some(e) = self.ensure_math(di) {
            self.math_err = e;
        } else {
            self.m_name.clear();
            self.m_expr.clear();
            self.m_rate.clear();
            self.m_unit.clear();
        }
    }

    fn preview_channel(&mut self) {
        self.preview.clear();
        let idx = match self.selected {
            Some(i) => i,
            None => return,
        };
        if let Some(e) = self.data(idx) {
            self.preview = e;
            return;
        }
        let v = &self.cache[&idx];
        let n = v.len().min(8);
        let vals: Vec<String> = v[..n].iter().map(|x| format!("{:.4}", x)).collect();
        let (mn, mx) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
        self.preview = format!("n={} min={:.4} max={:.4} first{}: {}", v.len(), mn, mx, n, vals.join(", "));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("files").resizable(true).show(ctx, |ui| {
            ui.heading("Log directory");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.dir);
                if ui.button("Refresh").clicked() { self.refresh_files(); }
            });
            if !self.dir_err.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.dir_err);
            }
            ui.separator();
            ui.heading(format!("Logs ({})", self.files.len()));
            egui::ScrollArea::vertical().show(ui, |ui| {
                let files = std::mem::take(&mut self.files);
                for p in &files {
                    let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.button(&name).clicked() { self.open(p.clone()); }
                }
                self.files = files;
            });
        });

        egui::SidePanel::right("inspector").resizable(true).show(ctx, |ui| {
            ui.heading("Channel inspector");
            if let Some(mname) = self.sel_math.clone() {
                if let Some(d) = self.maths.iter().find(|m| m.name == mname).cloned() {
                    egui::Grid::new("mathgrid").striped(true).show(ui, |ui| {
                        ui.label("name"); ui.monospace(&d.name); ui.end_row();
                        ui.label("expr"); ui.monospace(&d.expr); ui.end_row();
                        ui.label("unit"); ui.monospace(&d.unit); ui.end_row();
                        ui.label("req Hz"); ui.monospace(d.rate_hz.map(|r| r.to_string()).unwrap_or_else(|| "auto".into())); ui.end_row();
                    });
                    match self.math_data.get(&d.name) {
                        Some((rate, v)) => {
                            let (mn, mx) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
                            ui.monospace(format!("eval {rate}Hz n={} min={mn:.4} max={mx:.4}", v.len()));
                        }
                        None => { ui.monospace("not evaluated — see maths error"); }
                    }
                }
            } else {
                match (&self.loaded, self.selected) {
                (Some(l), Some(i)) if i < l.channels.len() => {
                    let c = &l.channels[i];
                    egui::Grid::new("chgrid").striped(true).show(ui, |ui| {
                        ui.label("name"); ui.monospace(&c.name); ui.end_row();
                        ui.label("short"); ui.monospace(&c.short_name); ui.end_row();
                        ui.label("unit"); ui.monospace(&c.unit); ui.end_row();
                        ui.label("datatype"); ui.monospace(format!("{:?}", c.datatype)); ui.end_row();
                        ui.label("rate Hz"); ui.monospace(c.sample_rate.to_string()); ui.end_row();
                        ui.label("samples"); ui.monospace(c.data_count.to_string()); ui.end_row();
                        ui.label("duration s"); ui.monospace(format!("{:.1}", c.data_count as f64 / c.sample_rate.max(1) as f64)); ui.end_row();
                        ui.label("conv"); ui.monospace(format!("off={} mul={} scale={} dec={}", c.offset, c.mul, c.scale, c.dec_places)); ui.end_row();
                        ui.label("data addr"); ui.monospace(format!("0x{:x}", c.data_addr)); ui.end_row();
                    });
                    if ui.button("Preview first samples").clicked() { self.preview_channel(); }
                    if !self.preview.is_empty() { ui.monospace(&self.preview); }
                }
                _ => { ui.label("Select a channel."); }
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("motec_rev B3 — plots + maths");
            if !self.load_err.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.load_err);
            }
            let (fname, nch, beacons, event) = match self.loaded.as_ref() {
                None => {
                    ui.label("Select an .ld file on the left.");
                    return;
                }
                Some(l) => (
                    l.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    l.channels.len(),
                    l.beacons.clone(),
                    l.event.clone(),
                ),
            };
            ui.horizontal(|ui| {
                ui.heading(&fname);
                ui.monospace(format!("{} ch", nch));
                if beacons.len() > 1 {
                    ui.monospace(format!("{} beacons / {} laps", beacons.len(), beacons.len() - 1));
                } else {
                    ui.monospace("no .ldx — full session");
                }
            });
            if let Some(e) = &event {
                ui.monospace(format!("event {} | session {}", e.name, e.session));
            }

            // ---- B3 maths editor ----
            ui.collapsing(format!("Maths channels ({}) — e.g. 'Steering Wheel Angle' * 2", self.maths.len()), |ui| {
                let defs: Vec<MathDef> = self.maths.clone();
                let mut remove: Option<usize> = None;
                for (di, d) in defs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut on = self.math_plotted.contains(&d.name);
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                if self.math_plotted.len() + self.plotted.len() < 12 {
                                    self.math_plotted.push(d.name.clone());
                                }
                            } else {
                                self.math_plotted.retain(|x| *x != d.name);
                            }
                        }
                        if ui.selectable_label(self.sel_math.as_deref() == Some(d.name.as_str()), &d.name).clicked() {
                            self.sel_math = Some(d.name.clone());
                            self.selected = None;
                            self.preview.clear();
                        }
                        ui.monospace(format!("= {}  [{}]", d.expr, d.unit));
                        if ui.small_button("x").clicked() {
                            remove = Some(di);
                        }
                    });
                }
                if let Some(di) = remove {
                    let gone = self.maths.remove(di);
                    self.math_plotted.retain(|x| *x != gone.name);
                    self.math_data.remove(&gone.name);
                    if self.sel_math.as_deref() == Some(gone.name.as_str()) {
                        self.sel_math = None;
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("name");
                    ui.text_edit_singleline(&mut self.m_name);
                    ui.label("=");
                    ui.text_edit_singleline(&mut self.m_expr);
                    ui.label("unit");
                    ui.text_edit_singleline(&mut self.m_unit);
                    ui.label("Hz");
                    ui.text_edit_singleline(&mut self.m_rate);
                    if ui.button("Add").clicked() {
                        self.add_math();
                    }
                });
                ui.monospace("names with spaces need 'quotes' | blank Hz = fastest input | see docs/MATHS_GUIDE.md");
                if !self.math_err.is_empty() {
                    ui.colored_label(egui::Color32::RED, &self.math_err);
                }
            });

            // ---- plot (top ~55%) ----
            let total = ui.available_size();
            let plot_h = (total.y * 0.55).clamp(200.0, (total.y - 140.0).max(200.0));
            let mut hover_text = String::new();
            ui.allocate_ui(egui::vec2(total.x, plot_h), |ui| {
                Plot::new("time_plot")
                    .legend(Legend::default())
                    .allow_zoom(true)
                    .allow_drag(true)
                    .allow_scroll(true)
                    .x_axis_label("time (s)")
                    .label_formatter(|name, val| format!("{name}\nt={:.3}s v={:.4}", val.x, val.y))
                    .show(ui, |plot_ui| {
                    let plotted: Vec<usize> = self.plotted.clone();
                    for (k, idx) in plotted.iter().enumerate() {
                        let ch = match self.loaded.as_ref().unwrap().channels.get(*idx) {
                            Some(c) => c.clone(),
                            None => continue,
                        };
                        if self.cache.contains_key(idx) {
                            let v = &self.cache[idx];
                            let rate = ch.sample_rate.max(1) as f64;
                            let step = (v.len() / 8000).max(1);
                            let pts: Vec<[f64; 2]> = v.iter().step_by(step).enumerate()
                                .map(|(i, y)| [(i * step) as f64 / rate, *y]).collect();
                            let color = PALETTE[k % PALETTE.len()];
                            plot_ui.line(Line::new(ch.name.clone(), PlotPoints::new(pts)).color(color));
                        }
                    }
                    for b in &beacons {
                        plot_ui.vline(VLine::new(format!("beacon {b:.1}s"), *b).color(egui::Color32::GRAY));
                    }
                    let mplot: Vec<String> = self.math_plotted.clone();
                    let base = self.plotted.len();
                    for (j, name) in mplot.iter().enumerate() {
                        if let Some((rate, v)) = self.math_data.get(name) {
                            let step = (v.len() / 8000).max(1);
                            let pts: Vec<[f64; 2]> = v.iter().step_by(step).enumerate()
                                .map(|(i, y)| [(i * step) as f64 / rate, *y]).collect();
                            let color = PALETTE[(base + j) % PALETTE.len()];
                            plot_ui.line(Line::new(format!("MATH {name}"), PlotPoints::new(pts)).color(color));
                        }
                    }
                    if let Some(pt) = plot_ui.pointer_coordinate() {
                        hover_text = format!("t={:.3}s", pt.x);
                    }
                });
            });
            if !hover_text.is_empty() {
                self.hover = hover_text;
            }
            ui.monospace(format!("{} plotted (max 12)  |  drag=pan  wheel=zoom  box-select=zoom  |  {}", self.plotted.len() + self.math_plotted.len(), self.hover));

            // ---- ensure plotted data cached ----
            let need: Vec<usize> = self.plotted.clone();
            for idx in need {
                if let Some(e) = self.data(idx) {
                    self.load_err = e;
                }
            }
            let mneed: Vec<usize> = (0..self.maths.len())
                .filter(|di| self.math_plotted.contains(&self.maths[*di].name))
                .collect();
            for di in mneed {
                if let Some(e) = self.ensure_math(di) {
                    self.math_err = e;
                }
            }

            // ---- channel list ----
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut self.filter);
                if ui.button("Clear plots").clicked() { self.plotted.clear(); }
            });
            let filt = self.filter.to_lowercase();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let log = self.loaded.as_ref().unwrap();
                for (i, c) in log.channels.iter().enumerate() {
                    if !filt.is_empty()
                        && !c.name.to_lowercase().contains(&filt)
                        && !c.unit.to_lowercase().contains(&filt) { continue; }
                    ui.horizontal(|ui| {
                        let mut on = self.plotted.contains(&i);
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                if self.plotted.len() < 12 { self.plotted.push(i); }
                            } else {
                                self.plotted.retain(|x| *x != i);
                            }
                        }
                        let label = format!("{:>4}  {:<34} {:>4}Hz {:>8}  [{}]",
                            i, c.name, c.sample_rate, c.data_count, c.unit);
                        if ui.selectable_label(self.selected == Some(i), label).clicked() {
                            self.selected = Some(i);
                            self.sel_math = None;
                            self.preview.clear();
                        }
                    });
                }
            });
        });
    }
}

fn print_summary(path: &PathBuf) -> Result<(), String> {
    let l = load_log(path)?;
    println!("{}: date={} time={} venue={} driver={} channels={}",
        path.display(), l.header.date_string, l.header.time_string, l.header.venue, l.header.driver, l.channels.len());
    if !l.beacons.is_empty() {
        println!("beacons (s): {}", l.beacons.iter().map(|b| format!("{:.3}", b)).collect::<Vec<_>>().join(", "));
    }
    for (i, c) in l.channels.iter().enumerate() {
        println!("{:>4}  {:<34} {:>4}Hz {:>8}  [{:?} off={} mul={} scale={} dec={} unit={}]",
            i, c.name, c.sample_rate, c.data_count, c.datatype, c.offset, c.mul, c.scale, c.dec_places, c.unit);
    }
    Ok(())
}

fn eval_cli(path: &PathBuf, expr: &str, rate_opt: Option<f64>) -> Result<(), String> {
    use std::collections::HashMap;
    let l = load_log(path)?;
    let refs = motec_ld::referenced_channels(expr).map_err(|e| e.to_string())?;
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut src = BufReader::new(f);
    let mut r = LDReader::new(&mut src);
    // decode only referenced channels into owned vecs, then borrow them
    let mut owned: HashMap<String, (f64, Vec<f64>)> = HashMap::new();
    for name in &refs {
        match l.channels.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
            Some(ch) => {
                let d = r.channel_data(ch).map_err(|e| format!("data: {e}"))?;
                let vals: Vec<f64> = d.iter().map(|s| s.decode_f64(ch)).collect();
                owned.insert(ch.name.clone(), (ch.sample_rate.max(1) as f64, vals));
            }
            None => {
                let avail: Vec<String> = l.channels.iter().map(|c| c.name.clone()).collect();
                return Err(motec_ld::MathError::UnknownChannel { name: name.clone(), available: avail }.to_string());
            }
        }
    }
    let inputs: HashMap<String, MathInput> = owned
        .iter()
        .map(|(k, (rate, v))| (k.clone(), MathInput { rate_hz: *rate, data: v }))
        .collect();
    let def = MathDef { name: "cli".into(), expr: expr.into(), rate_hz: rate_opt, unit: "".into() };
    let (rate, vals) = eval_math(&def, &inputs, &Default::default()).map_err(|e| e.to_string())?;
    let (mn, mx) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
    println!("expr: {expr}\nrate: {rate}Hz n={} min={mn:.4} max={mx:.4}", vals.len());
    for (i, v) in vals.iter().take(10).enumerate() {
        println!("t={:.3}s  {v:.4}", i as f64 / rate);
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--list" {
        let p = PathBuf::from(&args[2]);
        if let Err(e) = print_summary(&p) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() == 3 && args[1] == "--dir" {
        match list_ld_files(&args[2]) {
            Ok(f) => for p in f { println!("{}", p.display()); },
            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
        }
        return;
    }
    if args.len() >= 4 && args[1] == "--eval" {
        // motec-gui --eval <file.ld> '<expr>' [rate_hz]
        let p = PathBuf::from(&args[2]);
        let rate = args.get(4).map(|s| s.parse::<f64>().unwrap_or(f64::NAN));
        if let Err(e) = eval_cli(&p, &args[3], rate) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    let opts = eframe::NativeOptions::default();
    eframe::run_native("motec_rev B3", opts, Box::new(|_| Ok(Box::new(App::new())))).expect("gui");
}
