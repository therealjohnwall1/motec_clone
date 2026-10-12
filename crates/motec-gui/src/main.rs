use motec_ld::{ChannelMetadata, Event, Header, LDReader};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

struct LoadedLog {
    path: PathBuf,
    header: Header,
    event: Option<Event>,
    channels: Vec<ChannelMetadata>,
}

fn load_log(path: &PathBuf) -> Result<LoadedLog, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut src = BufReader::new(f);
    let mut r = LDReader::new(&mut src);
    let header = r.read_header().map_err(|e| format!("header: {e}"))?;
    let event = r.read_event().map_err(|e| format!("event: {e}"))?;
    let channels = r.read_channels().map_err(|e| format!("channels: {e}"))?;
    Ok(LoadedLog { path: path.clone(), header, event, channels })
}

fn list_ld_files(dir: &str) -> Result<Vec<PathBuf>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("read dir: {e}"))?;
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
    preview: String,
}

impl App {
    fn new() -> Self {
        let dir = String::from("/home/mechanism/stuff");
        let files = list_ld_files(&dir).unwrap_or_default();
        Self { dir, files, dir_err: String::new(), loaded: None, load_err: String::new(),
               filter: String::new(), selected: None, preview: String::new() }
    }

    fn refresh_files(&mut self) {
        match list_ld_files(&self.dir) {
            Ok(f) => { self.files = f; self.dir_err.clear(); }
            Err(e) => self.dir_err = e,
        }
    }

    fn open(&mut self, path: PathBuf) {
        match load_log(&path) {
            Ok(l) => { self.loaded = Some(l); self.load_err.clear(); self.selected = None; self.preview.clear(); }
            Err(e) => self.load_err = e,
        }
    }

    fn preview_channel(&mut self) {
        self.preview.clear();
        let (log, idx) = match (&self.loaded, self.selected) {
            (Some(l), Some(i)) if i < l.channels.len() => (l, i),
            _ => return,
        };
        let ch = &log.channels[idx];
        let f = match File::open(&log.path) {
            Ok(f) => f,
            Err(e) => { self.preview = format!("open: {e}"); return; }
        };
        let mut src = BufReader::new(f);
        let mut r = LDReader::new(&mut src);
        match r.channel_data(ch) {
            Ok(d) => {
                let n = d.len().min(8);
                let vals: Vec<String> = d[..n].iter().map(|s| format!("{:.4}", s.decode_f64(ch))).collect();
                self.preview = format!("n={} first{}: {}", d.len(), n, vals.join(", "));
            }
            Err(e) => self.preview = format!("data: {e}"),
        }
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
            ui.heading("motec_rev B0 — log + channels");
            if !self.load_err.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.load_err);
            }
            match &self.loaded {
                None => { ui.label("Select an .ld file on the left."); }
                Some(l) => {
                    let name = l.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    ui.heading(&name);
                    ui.monospace(format!("date {}  time {}  venue {}  driver {}",
                        l.header.date_string, l.header.time_string, l.header.venue, l.header.driver));
                    if let Some(e) = &l.event {
                        ui.monospace(format!("event {} | session {} | {}", e.name, e.session, e.comment));
                    }
                    ui.horizontal(|ui| {
                        ui.label("Filter:");
                        ui.text_edit_singleline(&mut self.filter);
                        ui.label(format!("{} channels", l.channels.len()));
                    });
                    let filt = self.filter.to_lowercase();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, c) in l.channels.iter().enumerate() {
                            if !filt.is_empty()
                                && !c.name.to_lowercase().contains(&filt)
                                && !c.unit.to_lowercase().contains(&filt) { continue; }
                            let label = format!("{:>4}  {:<34} {:>4}Hz {:>8}  [{}]",
                                i, c.name, c.sample_rate, c.data_count, c.unit);
                            if ui.selectable_label(self.selected == Some(i), label).clicked() {
                                self.selected = Some(i);
                                self.preview.clear();
                            }
                        }
                    });
                }
            }
        });
    }
}

fn print_summary(path: &PathBuf) -> Result<(), String> {
    let l = load_log(path)?;
    println!("{}: date={} time={} venue={} driver={} channels={}",
        path.display(), l.header.date_string, l.header.time_string, l.header.venue, l.header.driver, l.channels.len());
    if let Some(e) = &l.event {
        println!("event={} session={} comment={}", e.name, e.session, e.comment);
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
    eframe::run_native("motec_rev B0", opts, Box::new(|_| Ok(Box::new(App::new())))).expect("gui");
}
