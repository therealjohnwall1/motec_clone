use egui_plot::{Legend, Line, Plot, PlotPoints, VLine};
use motec_ld::{ChannelMetadata, Event, Header, LDReader};
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
}

impl App {
    fn new() -> Self {
        let dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| String::from("."));
        let files = list_ld_files(&dir).unwrap_or_default();
        Self { dir, files, dir_err: String::new(), loaded: None, load_err: String::new(),
               filter: String::new(), selected: None, plotted: vec![], cache: HashMap::new(),
               preview: String::new(), hover: String::new() }
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
                self.plotted.clear();
                self.cache.clear();
                self.preview.clear();
            }
            Err(e) => self.load_err = e,
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
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("motec_rev B2 — plots");
            if !self.load_err.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.load_err);
            }
            let Some(log) = self.loaded.as_ref() else {
                ui.label("Select an .ld file on the left.");
                return;
            };
            let name = log.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.heading(&name);
                ui.monospace(format!("{} ch", log.channels.len()));
                if log.beacons.len() > 1 {
                    ui.monospace(format!("{} beacons / {} laps", log.beacons.len(), log.beacons.len() - 1));
                } else {
                    ui.monospace("no .ldx — full session");
                }
            });
            if let Some(e) = &log.event {
                ui.monospace(format!("event {} | session {}", e.name, e.session));
            }

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
                        let Some(ch) = log.channels.get(*idx) else { continue };
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
                    for b in &log.beacons {
                        plot_ui.vline(VLine::new(format!("beacon {b:.1}s"), *b).color(egui::Color32::GRAY));
                    }
                    if let Some(pt) = plot_ui.pointer_coordinate() {
                        hover_text = format!("t={:.3}s", pt.x);
                    }
                });
            });
            if !hover_text.is_empty() {
                self.hover = hover_text;
            }
            ui.monospace(format!("{} plotted (max 12)  |  drag=pan  wheel=zoom  box-select=zoom  |  {}", self.plotted.len(), self.hover));

            // ---- ensure plotted data cached ----
            let need: Vec<usize> = self.plotted.clone();
            for idx in need {
                if let Some(e) = self.data(idx) {
                    self.load_err = e;
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
    let opts = eframe::NativeOptions::default();
    eframe::run_native("motec_rev B2", opts, Box::new(|_| Ok(Box::new(App::new())))).expect("gui");
}
