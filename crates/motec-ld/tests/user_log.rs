// B0 gate: validates the forked parser against the real user log.
// Runs only when MOTEC_TEST_LD points at the .ld file (61MB, kept out of git).
use motec_ld::LDReader;
use std::fs::File;
use std::io::BufReader;

#[test]
fn user_log_parses() {
    let path = match std::env::var("MOTEC_TEST_LD") {
        Ok(p) => p,
        Err(_) => return, // skip when log not supplied
    };
    let f = File::open(&path).unwrap();
    let mut src = BufReader::new(f);
    let mut r = LDReader::new(&mut src);
    let h = r.read_header().unwrap();
    assert_eq!(h.venue, "Norcal Shootout");
    let ch = r.read_channels().unwrap();
    assert_eq!(ch.len(), 587);
    // shift/offset must be honored (DC-DC Case Temp. shift=32)
    let dc = ch.iter().find(|c| c.name == "DC-DC Case Temp.").unwrap();
    assert_eq!(dc.offset, 32);
    let data = r.channel_data(dc).unwrap();
    assert_eq!(data.len(), 1063);
    let v = data[0].decode_f64(dc);
    assert!((v - 406.8).abs() < 1e-6, "got {}", v);
}
