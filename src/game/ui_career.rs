//! Karriären: erfarenhetspoäng, rang, prestationer och statistik. Port av js/ui/career.js
//! (och prestationerna från v7/v8/v10 som gäller 2D).
use super::ui_widgets::*;
use crate::engine::{rgb, Canvas, Color, FontKind, Fonts, Input, Key};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Antal veckor (1–14) och byggen (101–105)
const WEEK_COUNT: u32 = 14;
const BUILD_COUNT: u32 = 5;
/// XP för en upplåst prestation (som i spelet JS)
pub const ACH_XP: i64 = 50;

/// Rangtrösklar (xp, namn) som i JS
pub const RANKS: [(i64, &str); 6] = [
    (0, "Praktikant"),
    (250, "Junior tekniker"),
    (700, "Nätverkstekniker"),
    (1400, "Senior tekniker"),
    (2500, "Nätverksarkitekt"),
    (4000, "Krabbans ärkefiende"),
];

/// Prestationstabell: id, namn, beskrivning, ikonnyckel (pixelikon, se draw_icon)
const ACH_TABLE: &[(&str, &str, &str, &str)] = &[
    ("crab1", "Krabbfångare", "Fånga Krabban.", "crab"),
    ("crab5", "Krabbjägare", "Fånga Krabban fem gånger.", "trap"),
    ("coffee5", "Kaffetörstig", "Drick fem koppar kaffe.", "cup"),
    ("ping50", "Pingmästare", "Skicka 50 ping.", "signal"),
    ("cmd500", "Terminalfantast", "Skriv 500 kommandon.", "term"),
    ("walk2k", "Maratontekniker", "Gå två kilometer på jobbet.", "boot"),
    ("fix10", "Felsökare", "Lös tio fel.", "wrench"),
    ("fix27", "Hela felbiblioteket", "Lös 30 fel.", "book"),
    ("report20", "Dokumenterare", "Skriv 20 helt rätta felrapporter.", "note"),
    ("combo3", "Kombo", "Lös tre fel i rad, med högst tre minuter emellan.", "bolt"),
    ("cable3", "Kabeldetektiv", "Dra ur, sätt i eller byt kablar tre gånger.", "plug"),
    ("nohint", "Utan ledtrådar", "Klara en vecka utan ledtrådar.", "brain"),
    ("fast", "Snabb tekniker", "Klara en vecka på högst tio minuter.", "clock"),
    ("exam", "Examen klarad", "Klara en examen.", "cap"),
    ("allweeks", "Veckans hjälte", "Klara alla veckor.", "trophy"),
    ("stars27", "Stjärnsamlare", "Samla alla stjärnor.", "star"),
    ("build1", "Från kartongen", "Klara ett bygge från grunden.", "box"),
    ("buildall", "Nätbyggare", "Klara alla byggen från grunden.", "bricks"),
    ("week10", "Tunnelbyggare", "Klara vecka 10 (VPN och lastbalansering).", "lock"),
    ("df5", "MTU-detektiv", "Skicka fem ping med DF-biten satt.", "ruler"),
    ("crypto10", "Tunnelseende", "Titta på show crypto tio gånger.", "search"),
    ("curl10", "Lastbalanserad", "Hämta tio webbsidor med curl.", "scale"),
    ("trips10", "Pendlare", "Åk mellan Göteborg och Borås tio gånger.", "car"),
    ("hard", "Hårding", "Klara en vecka på svår nivå.", "muscle"),
    ("night", "Övertid", "Spela i en timme totalt.", "moon"),
    ("nmap", "Portskannare", "Skanna ett nät eller en enhet med nmap.", "search"),
    ("tdr", "Kabeltestare", "Kör ett kabeltest med test cable-diagnostics tdr.", "ruler"),
    ("loop", "Loopback", "Skapa ett loopback-interface.", "loop"),
    ("banner", "Skyltmakare", "Sätt en banner motd.", "sign"),
    ("alias", "Genvägar", "Skapa ett eget kortkommando med alias exec.", "bolt"),
    ("mtr", "Ruttspanare", "Mät förlust per hopp med mtr eller pathping.", "signal"),
    ("help50", "Frågvis", "Tryck ? i Cisco-terminalen 50 gånger.", "question"),
    ("save10", "Sparsam", "Spara konfigurationen tio gånger.", "disk"),
    ("showrun20", "Konfigläsare", "Titta på show running-config 20 gånger.", "note"),
    ("ssh5", "Fjärrtekniker", "Logga in med SSH fem gånger.", "key"),
    ("debug", "Avlusare", "Slå på debug och stäng av den igen med undebug all.", "bug"),
    ("range", "Portintervall", "Skriv en ACL-rad med range, gt eller lt.", "scale"),
    ("topo", "Kartläsare", "Öppna topologin i handboken.", "map"),
    ("cat", "Kattvän", "Klappa kontorskatten.", "paw"),
    ("ducks", "Ankmatare", "Mata ankorna i dammen.", "paw"),
    ("wget", "Nedladdare", "Spara en webbsida till disk med wget.", "disk"),
    ("storm", "Stormvakt", "Skydda en port mot broadcaststormar med storm-control.", "bolt"),
    ("env", "Hälsokontroll", "Kontrollera fläktar och temperatur med show environment.", "gear"),
    ("typo10", "Stavfelsjägare", "Låt terminalen föreslå rätt kommando tio gånger.", "question"),
    ("ch1", "Utmanare", "Klara en av veckans utmaningar.", "target"),
    ("ch10", "Tio av tio", "Klara tio veckoutmaningar.", "medal"),
    ("animals10", "Djurvän", "Klappa djur tio gånger.", "paw"),
    ("animals50", "Djurviskaren", "Klappa djur 50 gånger.", "heart"),
    ("hard3", "Svår vecka, igen", "Klara tre veckor på svår nivå.", "muscle"),
    ("coffee25", "Kaffebryggare", "Drick 25 koppar kaffe.", "cup"),
];

/// Villkoret för en prestation
fn cond(id: &str, st: &BTreeMap<String, f64>, p: &ProgressInfo) -> bool {
    let s = |k: &str| st.get(k).copied().unwrap_or(0.0);
    match id {
        "crab1" => s("crabs") >= 1.0,
        "crab5" => s("crabs") >= 5.0,
        "coffee5" => s("coffees") >= 5.0,
        "ping50" => s("pings") >= 50.0,
        "cmd500" => s("commands") >= 500.0,
        "walk2k" => s("dist") >= 2000.0,
        "fix10" => s("fixed") >= 10.0,
        "fix27" => s("fixed") >= 30.0,
        "report20" => s("perfect") >= 20.0,
        "combo3" => s("bestCombo") >= 3.0,
        "cable3" => s("cables") >= 3.0,
        "nohint" => s("noHintWeeks") >= 1.0,
        "fast" => s("fastWeeks") >= 1.0,
        "exam" => s("exams") >= 1.0,
        "allweeks" => (1..=WEEK_COUNT).all(|w| p.weeks.contains_key(&w)),
        "stars27" => (1..=WEEK_COUNT).map(|w| p.weeks.get(&w).copied().unwrap_or(0)).sum::<u32>() >= WEEK_COUNT * 3,
        "build1" => p.weeks.keys().any(|&k| k > 100),
        "buildall" => (101..=100 + BUILD_COUNT).all(|w| p.weeks.contains_key(&w)),
        "week10" => p.weeks.contains_key(&10),
        "df5" => s("dfPings") >= 5.0,
        "crypto10" => s("cryptoShows") >= 10.0,
        "curl10" => s("curls") >= 10.0,
        "trips10" => s("trips") >= 10.0,
        "hard" => s("hardWeeks") >= 1.0,
        "night" => s("playSec") >= 3600.0,
        "nmap" => s("nmaps") >= 1.0,
        "tdr" => s("tdrs") >= 1.0,
        "loop" => s("loops") >= 1.0,
        "banner" => s("banners") >= 1.0,
        "alias" => s("aliases") >= 1.0,
        "mtr" => s("mtrs") >= 1.0,
        "help50" => s("helps") >= 50.0,
        "save10" => s("saves") >= 10.0,
        "showrun20" => s("showRuns") >= 20.0,
        "ssh5" => s("sshLogins") >= 5.0,
        "debug" => s("undebugs") >= 1.0,
        "range" => s("aclRanges") >= 1.0,
        "topo" => s("topos") >= 1.0,
        "cat" => s("cats") >= 1.0,
        "ducks" => s("duckFeeds") >= 1.0,
        "wget" => s("wgets") >= 1.0,
        "storm" => s("storms") >= 1.0,
        "env" => s("envs") >= 1.0,
        "typo10" => s("typos") >= 10.0,
        "ch1" => s("challenges") >= 1.0,
        "ch10" => s("challenges") >= 10.0,
        "animals10" => s("animals") >= 10.0,
        "animals50" => s("animals") >= 50.0,
        "hard3" => s("hardWeeks") >= 3.0,
        "coffee25" => s("coffees") >= 25.0,
        _ => false,
    }
}

/// En prestation (id, namn, beskrivning, ikon)
#[derive(Clone, Debug)]
pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
}

/// Det spelet behöver veta om framstegen för prestationer som gäller veckor och byggen.
#[derive(Clone, Debug, Default)]
pub struct ProgressInfo {
    /// Klarade veckor/byggen (veckonummer -> stjärnor 0–3)
    pub weeks: BTreeMap<u32, u32>,
}

/// Sparas i save.json (serde).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Career {
    pub xp: i64,
    /// Statistik: "dist", "commands", "pings", "fixed", "reports", "perfect", "coffees", "crabs", "hints",
    /// "playSec", "cables", "bestCombo", "noHintWeeks", "fastWeeks", "exams", "trips", "dfPings",
    /// "cryptoShows", "curls", "animals" m.fl.
    pub stats: BTreeMap<String, f64>,
    /// Upplåsta prestationer: id -> sekunder sedan epoken
    pub ach: BTreeMap<String, u64>,
}

fn rank_index(xp: i64) -> usize {
    let mut i = 0;
    for (k, r) in RANKS.iter().enumerate() {
        if xp >= r.0 {
            i = k;
        }
    }
    i
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Career {
    /// Lägg till XP. Returnerar Some(ny rang) om rangen höjdes.
    pub fn add_xp(&mut self, n: i64, why: &str) -> Option<String> {
        let before = rank_index(self.xp);
        self.xp = (self.xp + n).max(0);
        let after = rank_index(self.xp);
        if after > before { Some(RANKS[after].1.to_string()) } else { None }
    }
    /// Öka en statistikräknare
    pub fn bump(&mut self, stat: &str, by: f64) {
        *self.stats.entry(stat.to_string()).or_insert(0.0) += by;
    }
    /// Sätt en räknare till max(gammalt, v)
    pub fn max_stat(&mut self, stat: &str, v: f64) {
        let e = self.stats.entry(stat.to_string()).or_insert(0.0);
        if v > *e {
            *e = v;
        }
    }
    /// Värdet på en räknare (0 om saknas)
    pub fn stat(&self, stat: &str) -> f64 {
        self.stats.get(stat).copied().unwrap_or(0.0)
    }
    /// Kontrollera prestationerna. Returnerar de som just låstes upp (och ger deras XP).
    pub fn check(&mut self, p: &ProgressInfo) -> Vec<Achievement> {
        let mut got = Vec::new();
        for a in Career::all_achievements() {
            if self.ach.contains_key(a.id) {
                continue;
            }
            if cond(a.id, &self.stats, p) {
                self.ach.insert(a.id.to_string(), now_secs());
                got.push(a);
            }
        }
        // Varje ny prestation ger XP (rangen uppdateras av nästa add_xp/rank)
        self.xp += ACH_XP * got.len() as i64;
        got
    }
    /// (rangens namn, xp för nuvarande rang, xp för nästa rang eller None)
    pub fn rank(&self) -> (&'static str, i64, Option<i64>) {
        let i = rank_index(self.xp);
        (RANKS[i].1, RANKS[i].0, RANKS.get(i + 1).map(|r| r.0))
    }
    pub fn all_achievements() -> Vec<Achievement> {
        ACH_TABLE.iter().map(|&(id, name, desc, icon)| Achievement { id, name, desc, icon }).collect()
    }
    pub fn unlocked(&self, id: &str) -> bool {
        self.ach.contains_key(id)
    }
    /// Andel (0–1) av nuvarande rangsteg
    pub fn rank_frac(&self) -> f32 {
        let (_, from, to) = self.rank();
        match to {
            Some(t) => ((self.xp - from) as f32 / (t - from) as f32).clamp(0.0, 1.0),
            None => 1.0,
        }
    }
}

// ---------------------------------------------------------------- Ikoner

/// 8x8-pixelikoner (x = ritas)
fn icon_map(name: &str) -> [&'static str; 8] {
    match name {
        "crab" => ["x......x", "xx.xx.xx", ".x.xx.x.", "..xxxx..", ".xxxxxx.", "x.xxxx.x", "..x..x..", ".x....x."],
        "trap" => ["..xxxx..", ".x....x.", "x.x..x.x", "x..xx..x", "x.x..x.x", "x..xx..x", ".x....x.", "..xxxx.."],
        "cup" => ["..x.x...", "...x.x..", ".xxxxxx.", ".xxxxxxx", ".xxxxx.x", ".xxxxx.x", "..xxxxx.", "........"],
        "signal" => ["..xxxx..", ".x....x.", "x..xx..x", "..x..x..", "....x...", "...xx...", "...xx...", "........"],
        "term" => ["xxxxxxxx", "x......x", "x.x....x", "x..x...x", "x.x..xxx", "x......x", "xxxxxxxx", "........"],
        "boot" => ["..xxx...", "..xxx...", "..xxx...", "..xxxx..", "..xxxxx.", ".xxxxxxx", "xxxxxxxx", "........"],
        "wrench" => ["..xx.xx.", ".xx..xx.", ".xxxxxx.", "..xxxx..", "...xx...", "...xx...", "..xx....", "..xx...."],
        "book" => ["xxxxxxx.", "xx....x.", "xx.xx.x.", "xx....x.", "xx.xx.x.", "xx....x.", "xxxxxxx.", "........"],
        "note" => ["xxxxxx..", "x....xx.", "x.xx.xxx", "x....x.x", "x.xxxx.x", "x......x", "xxxxxxxx", "........"],
        "bolt" => ["....xx..", "...xx...", "..xx....", ".xxxxxx.", "....xx..", "...xx...", "..xx....", "..x....."],
        "plug" => ["..x..x..", "..x..x..", ".xxxxxx.", ".xxxxxx.", "..xxxx..", "...xx...", "...xx...", "...xx..."],
        "brain" => [".xx..xx.", "xxxxxxxx", "xx.xx.xx", "xxxxxxxx", "xx.xx.xx", "xxxxxxxx", ".xx..xx.", "..x..x.."],
        "clock" => ["..xxxx..", ".x..x.x.", "x...x..x", "x...xx.x", "x......x", ".x....x.", "..xxxx..", "........"],
        "cap" => ["...xx...", "..xxxx..", ".xxxxxx.", "xxxxxxxx", ".xx..xx.", ".xx..xxx", "..xxxxx.", "........"],
        "trophy" => ["xxxxxxxx", "x.xxxx.x", "x.xxxx.x", ".xxxxxx.", "..xxxx..", "...xx...", "..xxxx..", ".xxxxxx."],
        "star" => ["...xx...", "...xx...", "xxxxxxxx", ".xxxxxx.", "..xxxx..", ".xxxxxx.", ".xx..xx.", "xx....xx"],
        "box" => ["xxxxxxxx", "x......x", "xxxxxxxx", "x..xx..x", "x..xx..x", "x......x", "x......x", "xxxxxxxx"],
        "bricks" => ["xxx.xxx.", "xxx.xxx.", "........", ".xxx.xxx", ".xxx.xxx", "........", "xxx.xxx.", "xxx.xxx."],
        "lock" => ["..xxxx..", ".x....x.", ".x....x.", "xxxxxxxx", "xxx..xxx", "xxx..xxx", "xxxxxxxx", "xxxxxxxx"],
        "ruler" => ["xxxxxxxx", "x.x.x.xx", "x.x.x.xx", "xxxxxxxx", "........", "........", "........", "........"],
        "search" => ["..xxx...", ".x...x..", "x.....x.", "x.....x.", ".x...x..", "..xxxxx.", ".....xxx", "......xx"],
        "scale" => ["...xx...", "xxxxxxxx", "..x..x..", ".x....x.", "xxxxxxxx", "........", "...xx...", "..xxxx.."],
        "car" => ["........", "..xxxx..", ".xxxxxx.", "xxxxxxxx", "xxxxxxxx", "xxxxxxxx", ".xx..xx.", ".xx..xx."],
        "muscle" => ["..xxx...", ".xxxxx..", ".xxxxxx.", "..xxxxxx", "...xxxxx", "..xxxxx.", "..xxxx..", "..xxx..."],
        "moon" => ["..xxx...", ".xx.....", "xx......", "xx......", "xx......", ".xx...xx", "..xxxxx.", "........"],
        "loop" => [".xxxxxx.", "x......x", "x.xxxx.x", "x.x..xxx", "x.xxxx..", "x.......", ".xxxxxx.", "........"],
        "sign" => ["xxxxxxxx", "xxxxxxxx", "xxxxxxxx", "xxxxxxxx", "...xx...", "...xx...", "...xx...", "...xx..."],
        "question" => ["..xxxx..", ".xx..xx.", ".....xx.", "....xx..", "...xx...", "........", "...xx...", "...xx..."],
        "disk" => ["xxxxxxx.", "x.xxx.xx", "x.xxx.xx", "x.....xx", "x.xxx.xx", "x.xxx.xx", "xxxxxxxx", "........"],
        "key" => [".xxx....", "x...x...", "x...x...", ".xxx....", "..x.....", "..xx....", "..x.....", "..xx...."],
        "bug" => ["x.xxxx.x", ".xxxxxx.", "xxxxxxxx", "x.xxxx.x", "x.xxxx.x", ".xxxxxx.", "x..xx..x", "........"],
        "map" => ["xxxxxxxx", "x.xx.xxx", "x.xx.x.x", "xxx..x.x", "xx.x.xxx", "xx.xx.xx", "xxxxxxxx", "........"],
        "paw" => ["x..xx..x", "xx.xx.xx", "x..xx..x", "..xxxx..", ".xxxxxx.", ".xxxxxx.", ".xx..xx.", "........"],
        "gear" => ["...xx...", ".x.xx.x.", "..xxxx..", "xxx..xxx", "xxx..xxx", "..xxxx..", ".x.xx.x.", "...xx..."],
        "target" => ["..xxxx..", ".x....x.", "x..xx..x", "x.xxxx.x", "x.xxxx.x", "x..xx..x", ".x....x.", "..xxxx.."],
        "medal" => ["x......x", ".x....x.", "..x..x..", "..xxxx..", ".xxxxxx.", ".xx..xx.", ".xxxxxx.", "..xxxx.."],
        "heart" => [".xx..xx.", "xxxxxxxx", "xxxxxxxx", "xxxxxxxx", ".xxxxxx.", "..xxxx..", "...xx...", "........"],
        _ => ["..xxxx..", ".xxxxxx.", "xxxxxxxx", "xxxxxxxx", "xxxxxxxx", "xxxxxxxx", ".xxxxxx.", "..xxxx.."],
    }
}

/// Ritar en ikon i valfri färg; scale = pixelstorlek (8 pixlar * scale)
pub fn draw_icon(c: &mut Canvas, name: &str, x: i32, y: i32, scale: i32, col: Color) {
    let m = icon_map(name);
    for (yy, row) in m.iter().enumerate() {
        for (xx, ch) in row.chars().enumerate() {
            if ch == 'x' {
                c.fill_rect(x + xx as i32 * scale, y + yy as i32 * scale, scale, scale, col);
            }
        }
    }
}

// ---------------------------------------------------------------- Dialogen

const DLG: Rect = Rect { x: 140, y: 30, w: 1000, h: 660 };
const HEAD_H: i32 = 126;
const FOOT_H: i32 = 62;
const CLOSE_BTN: Rect = Rect { x: DLG.x + DLG.w - 24 - 140, y: DLG.y + DLG.h - FOOT_H + 12, w: 140, h: 38 };

/// Statistikrader: nyckel, etikett, format (0 tal, 1 meter, 2 tid)
const STAT_ROWS: &[(&str, &str, u8)] = &[
    ("dist", "Gången sträcka", 1),
    ("commands", "Kommandon", 0),
    ("pings", "Ping", 0),
    ("fixed", "Lösta fel", 0),
    ("reports", "Felrapporter", 0),
    ("perfect", "Helt rätta rapporter", 0),
    ("coffees", "Koppar kaffe", 0),
    ("crabs", "Fångade krabbor", 0),
    ("animals", "Klappade djur", 0),
    ("hints", "Använda ledtrådar", 0),
    ("challenges", "Veckoutmaningar", 0),
    ("hardWeeks", "Svåra veckor", 0),
    ("noHintWeeks", "Veckor utan ledtråd", 0),
    ("fastWeeks", "Snabba veckor", 0),
    ("bestCombo", "Bästa kombo", 0),
    ("cables", "Kabelbyten", 0),
    ("trips", "Resor Gbg–Borås", 0),
    ("exams", "Examen klarade", 0),
    ("playSec", "Speltid", 2),
];

fn fmt_stat(v: f64, kind: u8) -> String {
    match kind {
        1 => if v >= 1000.0 { format!("{:.2} km", v / 1000.0) } else { format!("{} m", v.round() as i64) },
        2 => {
            let m = (v / 60.0).floor() as i64;
            if m >= 60 { format!("{} h {} min", m / 60, m % 60) } else { format!("{} min", m) }
        }
        _ => format!("{}", v.round() as i64),
    }
}

/// Karriärdialogen (J): rang, XP-stapel, prestationer (låsta/upplåsta) och statistik.
pub struct CareerView {
    pub closed: bool,
    scroll: i32,
    max_scroll: i32,
    mouse: (i32, i32),
}

impl CareerView {
    pub fn new() -> CareerView { CareerView { closed: false, scroll: 0, max_scroll: 0, mouse: (-1, -1) } }
    /// Esc/J/klick på Stäng sätter closed
    pub fn update(&mut self, input: &Input) {
        self.mouse = (input.mouse_x, input.mouse_y);
        if input.was_pressed(Key::Escape) || input.was_pressed(Key::J) {
            self.closed = true;
        }
        if input.clicked && CLOSE_BTN.contains(input.mouse_x, input.mouse_y) {
            self.closed = true;
        }
        let mut d = 0;
        if input.wheel != 0.0 { d -= (input.wheel * 40.0) as i32; }
        for k in &input.pressed {
            match k {
                Key::Down => d += 40,
                Key::Up => d -= 40,
                Key::PageDown => d += 300,
                Key::PageUp => d -= 300,
                Key::Home => d -= 100000,
                Key::End => d += 100000,
                _ => {}
            }
        }
        if d != 0 {
            self.scroll = (self.scroll + d).clamp(0, self.max_scroll);
        }
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, career: &Career, p: &ProgressInfo) {
        c.dim(rgb(0, 0, 0), 150);
        panel(c, DLG, PANEL, BORDER);
        c.rect(DLG.x + 4, DLG.y + 4, DLG.w - 8, DLG.h - 8, 1, PANEL2);
        let x0 = DLG.x + 28;
        let cw = DLG.w - 56;

        // Rubrik
        fonts.text(c, x0, DLG.y + 22, "KARRIÄR", TITLE + 4.0, ACCENT, FontKind::Pixel);
        let all = Career::all_achievements();
        let done = all.iter().filter(|a| career.unlocked(a.id)).count();
        let cnt = format!("{}/{} prestationer", done, all.len());
        let w = text_w(fonts, &cnt, BODY, FontKind::Mono);
        fonts.text(c, x0 + cw - w, DLG.y + 22, &cnt, BODY, MUTED, FontKind::Mono);

        // Rang och XP-stapel
        let (rname, from, to) = career.rank();
        let ry = DLG.y + 54;
        draw_icon(c, "medal", x0, ry + 2, 4, ACCENT);
        fonts.text(c, x0 + 44, ry, rname, BODY + 6.0, TEXT, FontKind::Mono);
        let xps = match to {
            Some(t) => format!("{} / {} XP  –  nästa: {}", career.xp, t, next_name(from)),
            None => format!("{} XP  –  högsta rangen!", career.xp),
        };
        let w = text_w(fonts, &xps, BODY, FontKind::Mono);
        fonts.text(c, x0 + cw - w, ry + 4, &xps, BODY, MUTED, FontKind::Mono);
        bar(c, x0 + 44, ry + 36, cw - 44, 16, career.rank_frac());
        c.fill_rect(DLG.x + 12, DLG.y + HEAD_H - 4, DLG.w - 24, 2, BORDER);

        // Innehåll (scrollas)
        let vy0 = DLG.y + HEAD_H;
        let vy1 = DLG.y + DLG.h - FOOT_H;
        let vh = vy1 - vy0;
        c.set_clip(DLG.x + 4, vy0, DLG.w - 8, vh);
        let sb = 14;
        let iw = cw - sb;
        let mut y = vy0 + 10 - self.scroll;

        fonts.text(c, x0, y, "PRESTATIONER", TITLE - 2.0, ACCENT, FontKind::Pixel);
        y += 30;
        let colw = (iw - 12) / 2;
        let ch = 62;
        for (i, a) in all.iter().enumerate() {
            let cx = x0 + (i as i32 % 2) * (colw + 12);
            let cy = y + (i as i32 / 2) * (ch + 8);
            if cy + ch < vy0 || cy > vy1 { continue; }
            let got = career.unlocked(a.id);
            let r = Rect::new(cx, cy, colw, ch);
            panel(c, r, if got { PANEL2 } else { BG }, if got { ACCENT } else { rgb(0x35, 0x3a, 0x55) });
            let ib = Rect::new(cx + 8, cy + 9, 44, 44);
            c.fill_rect(ib.x, ib.y, ib.w, ib.h, if got { rgb(0x3a, 0x34, 0x18) } else { rgb(0x16, 0x18, 0x24) });
            if got {
                draw_icon(c, a.icon, ib.x + 6, ib.y + 6, 4, ACCENT);
            } else {
                draw_icon(c, a.icon, ib.x + 6, ib.y + 6, 4, rgb(0x3a, 0x3f, 0x5a));
            }
            let tx = cx + 64;
            let tw = colw - 74;
            text_fit(c, fonts, tx, cy + 6, a.name, BODY + 2.0, if got { TEXT } else { MUTED }, FontKind::Mono, tw - if got { 22 } else { 0 });
            if got {
                fonts.text(c, cx + colw - 26, cy + 6, "✓", BODY, GOOD, FontKind::Mono);
            }
            let lines = fonts.wrap(a.desc, SMALL, FontKind::Mono, tw);
            for (li, l) in lines.iter().take(2).enumerate() {
                fonts.text(c, tx, cy + 28 + li as i32 * line_h(SMALL) - 2, l, SMALL, if got { MUTED } else { rgb(0x6c, 0x72, 0x92) }, FontKind::Mono);
            }
        }
        let rows = (all.len() as i32 + 1) / 2;
        y += rows * (ch + 8) + 14;

        fonts.text(c, x0, y, "STATISTIK", TITLE - 2.0, ACCENT, FontKind::Pixel);
        y += 30;
        let srows = (STAT_ROWS.len() as i32 + 1) / 2;
        for (i, &(k, label, kind)) in STAT_ROWS.iter().enumerate() {
            // Fyll kolumn för kolumn
            let col = i as i32 / srows;
            let row = i as i32 % srows;
            let cx = x0 + col * (colw + 12);
            let cy = y + row * 30;
            if cy + 30 < vy0 || cy > vy1 { continue; }
            if row % 2 == 0 {
                c.fill_rect(cx, cy, colw, 28, rgb(0x20, 0x24, 0x3a));
            }
            fonts.text(c, cx + 8, cy + 3, label, BODY, MUTED, FontKind::Mono);
            let v = fmt_stat(career.stat(k), kind);
            let w = text_w(fonts, &v, BODY, FontKind::Mono);
            fonts.text(c, cx + colw - 8 - w, cy + 3, &v, BODY, TEXT, FontKind::Mono);
        }
        y += srows * 30 + 14;
        let content_h = y + self.scroll - (vy0 + 10);
        c.reset_clip();

        self.max_scroll = (content_h + 10 - vh).max(0);
        self.scroll = self.scroll.clamp(0, self.max_scroll);
        // Rullningslist
        if self.max_scroll > 0 {
            let tx = x0 + cw - 8;
            c.fill_rect(tx, vy0 + 6, 8, vh - 12, BG);
            let th = ((vh - 12) as f32 * vh as f32 / (content_h + 10) as f32).max(30.0) as i32;
            let ty = vy0 + 6 + ((vh - 12 - th) as f32 * self.scroll as f32 / self.max_scroll as f32) as i32;
            c.fill_rect(tx, ty, 8, th, BORDER);
        }

        // Fot
        c.fill_rect(DLG.x + 12, vy1, DLG.w - 24, 2, BORDER);
        fonts.text(c, x0, vy1 + 20, "Rulla med mushjul eller piltangenter  –  Esc eller J stänger", SMALL, MUTED, FontKind::Mono);
        let hover = CLOSE_BTN.contains(self.mouse.0, self.mouse.1);
        button(c, fonts, CLOSE_BTN, "Stäng", true, hover, false);
    }

}

fn next_name(from: i64) -> &'static str {
    let i = RANKS.iter().position(|r| r.0 == from).unwrap_or(0);
    RANKS.get(i + 1).map(|r| r.1).unwrap_or("")
}

/// Stapel med fyllnad 0–1
fn bar(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, frac: f32) {
    c.fill_rect(x, y, w, h, BG);
    let fw = ((w - 4) as f32 * frac.clamp(0.0, 1.0)) as i32;
    c.fill_rect(x + 2, y + 2, fw, h - 4, ACCENT);
    if fw > 4 {
        c.fill_rect(x + 2, y + 2, fw, 3, rgb(0xff, 0xea, 0x9a));
    }
    c.rect(x, y, w, h, 2, BORDER);
}

/// XP-stapeln i HUD:en (liten, t.ex. under veckorubriken)
pub fn draw_xp_bar(c: &mut Canvas, fonts: &mut Fonts, x: i32, y: i32, w: i32, career: &Career) {
    let (name, _, to) = career.rank();
    let label = match to {
        Some(t) => format!("{}  {}/{} XP", name, career.xp, t),
        None => format!("{}  {} XP", name, career.xp),
    };
    text_fit(c, fonts, x, y, &label, SMALL, TEXT, FontKind::Mono, w);
    bar(c, x, y + line_h(SMALL) + 2, w, 10, career.rank_frac());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_och_rang() {
        let mut c = Career::default();
        assert_eq!(c.rank().0, "Praktikant");
        assert_eq!(c.add_xp(100, "x"), None);
        assert_eq!(c.add_xp(150, "x").as_deref(), Some("Junior tekniker"));
        assert_eq!(c.rank(), ("Junior tekniker", 250, Some(700)));
        c.add_xp(-10000, "x");
        assert_eq!(c.xp, 0);
        c.add_xp(5000, "x");
        assert_eq!(c.rank(), ("Krabbans ärkefiende", 4000, None));
    }

    #[test]
    fn prestationer() {
        let mut c = Career::default();
        let p = ProgressInfo::default();
        assert!(c.check(&p).is_empty());
        c.bump("crabs", 1.0);
        c.bump("coffees", 5.0);
        let got = c.check(&p);
        let ids: Vec<_> = got.iter().map(|a| a.id).collect();
        assert_eq!(ids, vec!["crab1", "coffee5"]);
        assert_eq!(c.xp, 100);
        assert!(c.check(&p).is_empty());
        c.max_stat("bestCombo", 2.0);
        c.max_stat("bestCombo", 3.0);
        c.max_stat("bestCombo", 1.0);
        assert_eq!(c.stat("bestCombo"), 3.0);
        assert!(c.check(&p).iter().any(|a| a.id == "combo3"));
        let mut p2 = ProgressInfo::default();
        for w in 1..=WEEK_COUNT { p2.weeks.insert(w, 3); }
        let ids: Vec<_> = c.check(&p2).iter().map(|a| a.id).collect();
        assert!(ids.contains(&"allweeks") && ids.contains(&"stars27") && ids.contains(&"week10"));
        p2.weeks.insert(101, 1);
        assert!(c.check(&p2).iter().any(|a| a.id == "build1"));
    }

    #[test]
    fn unika_id_och_serde() {
        let all = Career::all_achievements();
        let mut ids: Vec<_> = all.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        let mut c = Career::default();
        c.bump("animals", 10.0);
        c.check(&ProgressInfo::default());
        let j = serde_json::to_string(&c).unwrap();
        let d: Career = serde_json::from_str(&j).unwrap();
        assert_eq!(d.stat("animals"), 10.0);
        assert!(d.unlocked("animals10"));
    }

    #[test]
    fn rita_dialog() {
        let mut fonts = Fonts::load();
        let mut cv = Canvas::new(1280, 720);
        cv.clear(rgb(0x30, 0x60, 0x40));
        let mut c = Career::default();
        for (k, v) in [("dist", 2450.0), ("commands", 530.0), ("pings", 61.0), ("fixed", 12.0), ("crabs", 2.0),
                       ("coffees", 7.0), ("animals", 14.0), ("playSec", 4000.0), ("perfect", 3.0), ("reports", 8.0)] {
            c.bump(k, v);
        }
        c.add_xp(860, "x");
        let mut p = ProgressInfo::default();
        p.weeks.insert(1, 3);
        c.check(&p);
        let mut v = CareerView::new();
        v.draw(&mut cv, &mut fonts, &c, &p);
        crate::engine::save_png(&cv, "/tmp/karriar.png").unwrap();
        // scrolla ned
        let mut inp = Input::default();
        inp.pressed.push(Key::End);
        v.update(&inp);
        assert!(v.scroll > 0);
        let mut cv2 = Canvas::new(1280, 720);
        v.draw(&mut cv2, &mut fonts, &c, &p);
        crate::engine::save_png(&cv2, "/tmp/karriar2.png").unwrap();
        let mut inp = Input::default();
        inp.pressed.push(Key::J);
        v.update(&inp);
        assert!(v.closed);
        let mut bar = Canvas::new(300, 60);
        draw_xp_bar(&mut bar, &mut fonts, 10, 10, 200, &c);
    }
}
