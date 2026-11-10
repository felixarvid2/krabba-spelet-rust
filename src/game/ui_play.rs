//! Spelet runt världen: XP och karriär, kaffe, krabban, djur, veckoutmaningar, kartan och svårt läge
//! (där ett löst fel måste bevisas med ett kommando). Port av delar av js/game.js och js/ui/more.js.
use super::levels::{self, ChallengeCtx};
use super::ui_career::ProgressInfo;
use super::ui_map::{MapAct, MapView};
use super::{App, Overlay, ToastKind};
use crate::net::sim as S;

/// Kaffets verkan: fart och längd (sekunder)
pub const COFFEE_MULT: f64 = 1.35;
pub const COFFEE_SECS: f64 = 60.0;

/// En liten rolig notis per djur (nyckelord i djurets namn, text)
const ANIMALS: &[(&[&str], &str)] = &[
    (&["katt", "cat"], "Du kliar katten bakom örat. Den spinner nöjt: purr…"),
    (&["ank", "duck"], "Du strör ut några brödsmulor till ankorna. Kvack kvack!"),
    (&["duv", "pigeon"], "Du strör ut några smulor. Duvorna kommer genast."),
    (&["sture", "dammsug", "robot", "vacuum"], "Dammsugarroboten Städ-Sture piper glatt och fortsätter städa."),
    (&["ekorre", "squirrel"], "Ekorren stannar mitt i språnget, tittar på dig och försvinner uppför trädet."),
    (&["hund", "dog"], "Hunden viftar på svansen. Den vill ha en kluring, inte ett ping."),
    (&["fisk", "fish"], "Fisken blubbar förnöjt. Den har ingen aning om att nätet var nere."),
    (&["fågel", "bird", "mås", "masen"], "Fågeln kvittrar något som låter som en DHCP-förfrågan."),
    (&["kanin", "rabbit"], "Kaninen tvättar öronen och låtsas att den inte såg dig."),
    (&["groda", "frog"], "Grodan hoppar plums! i dammen."),
    (&["igelkott", "hedgehog"], "Igelkotten rullar ihop sig. Du lämnar den ifred."),
    (&["fjäril", "butterfly"], "Fjärilen landar på din axel en sekund och flyger vidare."),
];

pub fn animal_text(name: &str) -> String {
    let n = name.to_lowercase();
    for (keys, text) in ANIMALS {
        if keys.iter().any(|k| n.contains(k)) {
            return text.to_string();
        }
    }
    format!("Du hälsar på {}. Den verkar nöjd.", name)
}

impl App {
    pub fn hard(&self) -> bool {
        self.save.settings.difficulty == "hard"
    }

    pub fn apply_weather(&mut self) {
        let w = if self.save.settings.weather == "sol" { "sol" } else { "auto" };
        if let Some(r) = &mut self.run {
            r.world.set_weather(w);
        }
    }

    pub fn stat(&mut self, k: &str) {
        self.save.career.bump(k, 1.0);
    }

    /// Klarade veckor (stjärnor) för prestationerna
    pub fn progress_info(&self) -> ProgressInfo {
        ProgressInfo { weeks: self.save.progress.iter().filter(|(_, p)| p.stars > 0).map(|(k, p)| (*k, p.stars)).collect() }
    }

    // ------------------------------------------------------------------ XP

    /// Ger (eller drar av) XP. Svårt läge ger ×1.5, lätt ×0.75 (som JS). Visar en notis om `why` inte är tom.
    pub fn xp(&mut self, n: i64, why: &str) {
        let n = if n > 0 {
            match self.save.settings.difficulty.as_str() {
                "hard" => (n as f64 * 1.5).round() as i64,
                "easy" => (n as f64 * 0.75).round() as i64,
                _ => n,
            }
        } else {
            n
        };
        if n == 0 {
            return;
        }
        let up = self.save.career.add_xp(n, why);
        if n > 0 {
            crate::engine::audio::sfx(if up.is_some() { "levelUp" } else { "xp" });
        }
        if let Some(r) = &mut self.run {
            r.x.week_xp += n;
        }
        if !why.is_empty() {
            let t = format!("{}{} XP · {}", if n > 0 { "+" } else { "−" }, n.abs(), why);
            self.toasts.push(super::Toast { text: t, ttl: 3.5, kind: if n > 0 { ToastKind::Good } else { ToastKind::Warn } });
            if self.toasts.len() > 5 {
                self.toasts.remove(0);
            }
        }
        if let Some(rank) = up {
            self.toast(format!("Ny rang: {}!", rank), ToastKind::Good);
        }
    }

    /// Kollar prestationerna och visar notiser för nya
    pub fn career_check(&mut self) {
        let p = self.progress_info();
        let before = self.save.career.rank().0;
        let got = self.save.career.check(&p);
        if !got.is_empty() {
            crate::engine::audio::sfx("achievement");
        }
        for a in got {
            self.toasts.push(super::Toast { text: format!("Prestation: {} – {}", a.name, a.desc), ttl: 8.0, kind: ToastKind::Good });
            if self.toasts.len() > 5 {
                self.toasts.remove(0);
            }
        }
        let after = self.save.career.rank().0;
        if before != after {
            self.toast(format!("Ny rang: {}!", after), ToastKind::Good);
        }
    }

    /// Sekundtick för karriären: speltid, kaffet tar slut, utmaningar och prestationer
    pub fn career_tick(&mut self) {
        let mut over = false;
        if let Some(r) = &mut self.run {
            self.save.career.bump("playSec", 1.0);
            if r.x.boost_until > 0.0 && r.elapsed >= r.x.boost_until {
                r.x.boost_until = 0.0;
                r.world.set_speed_mult(1.0);
                over = true;
            }
        }
        if over {
            self.toast("Kaffet har slutat verka.", ToastKind::Info);
        }
        self.live_challenge();
        // prestationerna kollas bara när statistiken eller XP ändrats (inte för dist/playSec)
        let fp: f64 = self.save.career.xp as f64 + self.save.career.stats.iter().filter(|(k, _)| k.as_str() != "dist" && k.as_str() != "playSec").map(|(_, v)| *v).sum::<f64>();
        let changed = self.run.as_ref().map(|r| (r.x.career_fp - fp).abs() > 1e-9).unwrap_or(false);
        if changed {
            if let Some(r) = &mut self.run {
                r.x.career_fp = fp;
            }
            self.career_check();
        }
    }

    /// Statistik och förstagångsbonus för ett inskrivet kommando
    pub fn count_command(&mut self, line: &str) {
        self.save.career.bump("commands", 1.0);
        let low = line.trim().to_lowercase();
        let toks: Vec<&str> = low.split_whitespace().collect();
        let w = toks.first().copied().unwrap_or("");
        let fam = match w {
            "show" | "sh" => "show",
            "ping" => "ping",
            "traceroute" | "tracert" => "traceroute",
            "configure" | "conf" => "configure",
            "write" | "copy" => "write",
            "ipconfig" => "ipconfig",
            "nslookup" => "nslookup",
            "ssh" => "ssh",
            "debug" => "debug",
            "clear" => "clear",
            "curl" => "curl",
            "tracepath" => "tracepath",
            "crypto" => "crypto",
            _ => "",
        };
        if !fam.is_empty() {
            let k = format!("first:{}", fam);
            if !self.save.career.stats.contains_key(&k) {
                self.save.career.stats.insert(k, 1.0);
                self.xp(10, &format!("Nytt kommando: {}", fam));
            }
        }
        if w == "ping" {
            self.stat("pings");
            if toks.iter().any(|t| *t == "-f" || *t == "df-bit") || low.contains("-m do") {
                self.stat("dfPings");
            }
        }
        // (do )?sh\w* cry...
        let t2: Vec<&str> = if toks.first() == Some(&"do") { toks[1..].to_vec() } else { toks.clone() };
        if t2.len() >= 2 && t2[0].starts_with("sh") && t2[1].starts_with("cry") {
            self.stat("cryptoShows");
        }
        if w == "curl" || w == "wget" || w == "for" {
            self.stat("curls");
        }
        // fler statistiknycklar för prestationerna
        let extra: &[(&str, &str)] = &[("wget", "wgets"), ("nmap", "nmaps"), ("mtr", "mtrs"), ("banner", "banners"), ("alias", "aliases"), ("undebug", "undebugs"), ("ssh", "sshLogins"), ("help", "helps")];
        for (cmd, key) in extra {
            if w == *cmd {
                self.stat(key);
            }
        }
        if t2.len() >= 2 && t2[0].starts_with("sh") && t2[1].starts_with("run") {
            self.stat("showRuns");
        }
        if low.starts_with("write") || low.starts_with("wr ") || low == "wr" || low.starts_with("copy run") || low.starts_with("copy ru") {
            self.stat("saves");
        }
        if low.starts_with("no debug") || low.starts_with("no deb") {
            self.stat("undebugs");
        }
        if low.contains('?') {
            self.stat("helps");
        }
    }

    pub fn on_hint(&mut self) {
        self.stat("hints");
        if self.save.settings.difficulty != "easy" {
            self.xp(-15, "Ledtråd");
        }
    }

    pub fn on_report(&mut self, score: u32) {
        self.stat("reports");
        if score == 2 {
            self.stat("perfect");
            self.xp(60, "Helt rätt felrapport");
        } else if score == 1 {
            self.xp(25, "Felrapport");
        }
    }

    // ------------------------------------------------------------------ löst fel

    /// Uppgift `i` är löst (i svårt läge: bevisat). Ger toast, XP och kombo.
    pub fn confirm_task(&mut self, i: usize, proven: bool) {
        let (title, build, left, hints) = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            let t = match run.week.as_ref() {
                Some(w) => w.tasks[i].clone(),
                None => return,
            };
            run.ts[i].fixed = true;
            run.ts[i].known = true;
            run.last_progress = run.elapsed;
            if t.build {
                run.ts[i].reported = true;
                run.ts[i].score = 2;
            }
            let left = run.ts.iter().filter(|s| !s.fixed).count();
            (t.title, t.build, left, run.ts[i].hints)
        };
        let msg = if build {
            format!("{}{} är klart!{}", if proven { "Bevisat! " } else { "" }, title, if left > 0 { format!(" {} steg kvar i bygget.", left) } else { String::new() })
        } else if proven {
            format!("Bevisat! {} är löst. Skriv felrapporten med F.", title)
        } else {
            format!("{} är löst! Skriv felrapporten med F.", title)
        };
        crate::engine::audio::sfx("success");
        self.toast(msg, ToastKind::Good);
        // XP och kombo
        self.stat("fixed");
        let combo = {
            let r = self.run.as_mut().unwrap();
            let now = r.elapsed;
            r.x.combo = match r.x.last_fix_at {
                Some(t) if now - t < 180.0 => r.x.combo + 1,
                _ => 1,
            };
            r.x.last_fix_at = Some(now);
            r.x.combo
        };
        self.save.career.max_stat("bestCombo", combo as f64);
        self.xp(100 + if hints == 0 { 50 } else { 0 }, if hints == 0 { "Fel löst utan ledtråd" } else { "Fel löst" });
        if combo >= 2 {
            self.xp(40 * (combo as i64 - 1), &format!("Kombo ×{}", combo));
        }
        self.refresh_markers();
    }

    /// Veckan klar: XP, statistik och utmaningen
    pub fn on_week_done(&mut self, res: &super::WeekResult) {
        let (exam, n, handled) = match &self.run {
            Some(r) => (r.exam, r.n, r.x.week_done),
            None => return,
        };
        if handled {
            return;
        }
        if let Some(r) = &mut self.run {
            r.x.week_done = true;
        }
        self.stat("weeks");
        if res.hints == 0 {
            self.stat("noHintWeeks");
        }
        if res.minutes <= 10 {
            self.stat("fastWeeks");
        }
        if exam {
            self.stat("exams");
        }
        if self.hard() {
            self.stat("hardWeeks");
        }
        self.xp(200 + 50 * res.stars as i64, "Veckan klar");
        if res.minutes <= 15 {
            self.xp(100, "Klar före fikat");
        }
        if let Some((_, true)) = res.challenge {
            self.mark_challenge(n);
        }
        self.career_check();
    }

    // ------------------------------------------------------------------ veckoutmaningar

    /// (titel, klarar spelaren den just nu?) – None utan utmaning eller i examen
    pub fn challenge_status(&self) -> Option<(&'static str, bool)> {
        let run = self.run.as_ref()?;
        if run.exam {
            return None;
        }
        let title = levels::challenge_title(run.n)?;
        let ok = if self.challenge_marked(run.n) {
            true
        } else {
            let ctx = ChallengeCtx {
                cmd_log: &run.x.chal_log,
                commands: run.commands as usize,
                any_hints: run.ts.iter().any(|t| t.hints > 0),
                elapsed_ms: run.elapsed * 1000.0,
                monitor_first: run.x.monitor_first,
                all_reports_perfect: run.ts.iter().all(|t| t.score == 2),
            };
            levels::challenge_done(run.n, &ctx)
        };
        Some((title, ok))
    }

    pub fn challenge_marked(&self, week: u32) -> bool {
        self.save.progress.get(&week).map(|p| p.challenge).unwrap_or(false)
    }

    pub fn mark_challenge(&mut self, week: u32) {
        if self.challenge_marked(week) {
            return;
        }
        self.save.progress.entry(week).or_default().challenge = true;
        self.stat("challenges");
        let t = levels::challenge_title(week).unwrap_or("");
        self.toast(format!("Veckans utmaning klar: {}", t), ToastKind::Good);
        self.xp(75, "Veckans utmaning");
        self.save.save();
    }

    /// Utmaningar som bara kräver att kommandon används går att avgöra medan veckan pågår
    fn live_challenge(&mut self) {
        let n = match &self.run {
            Some(r) if !r.done => r.n,
            _ => return,
        };
        if n == 0 || !levels::challenge_need(n) || self.challenge_marked(n) {
            return;
        }
        if let Some((_, true)) = self.challenge_status() {
            self.mark_challenge(n);
        }
    }

    // ------------------------------------------------------------------ kaffe, krabba och djur

    pub fn drink_coffee(&mut self) {
        let boosted = self.run.as_ref().map(|r| r.x.boost_until > r.elapsed).unwrap_or(false);
        if boosted {
            self.toast("Du har redan kaffe i kroppen. Vänta lite.", ToastKind::Info);
            return;
        }
        let first = {
            let r = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            r.x.boost_until = r.elapsed + COFFEE_SECS;
            r.world.set_speed_mult(COFFEE_MULT);
            let f = !r.x.coffee_xp;
            r.x.coffee_xp = true;
            f
        };
        crate::engine::audio::sfx("pour");
        self.stat("coffees");
        self.toast(format!("Kaffe! Du går {} % snabbare i {} sekunder.", ((COFFEE_MULT - 1.0) * 100.0).round(), COFFEE_SECS as i32), ToastKind::Good);
        if first {
            self.xp(5, "Kaffepaus");
        }
    }

    /// Gömmer Krabban nära veckans första Krabba-fel
    pub fn place_crab(&mut self) {
        let run = match &mut self.run {
            Some(r) => r,
            None => return,
        };
        if run.x.crab_caught {
            run.world.set_crab(None);
            return;
        }
        let near = run.week.as_ref().and_then(|w| {
            let open = w.tasks.iter().enumerate().find(|(i, t)| t.krabba && !run.ts[*i].fixed);
            open.or_else(|| w.tasks.iter().enumerate().find(|(_, t)| t.krabba)).map(|(_, t)| t.target.to_string())
        });
        run.world.set_crab(near);
    }

    pub fn catch_crab(&mut self) {
        let note = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            if run.x.crab_caught {
                return;
            }
            run.x.crab_caught = true;
            let idx = run.week.as_ref().and_then(|w| {
                let open = w.tasks.iter().enumerate().find(|(i, t)| t.krabba && !run.ts[*i].fixed);
                open.map(|(i, _)| i)
            });
            match (idx, run.week.as_ref()) {
                (Some(i), Some(w)) => {
                    // gratis ledtråd: felet blir känt och lappen avslöjar enheten
                    run.ts[i].known = true;
                    let target = w.tasks[i].target;
                    let name = run.st.devices.get(target).map(|d| d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| if d.label.is_empty() { target.to_string() } else { d.label.clone() })).unwrap_or_else(|| target.to_string());
                    format!(" Den tappade en lapp: \"Jag har pillat på {}…\"", name)
                }
                _ => " Den verkar ha varit sysslolös den här veckan.".to_string(),
            }
        };
        crate::engine::audio::sfx("success");
        self.stat("crabs");
        self.toast(format!("Du fångade Krabban!{}", note), ToastKind::Good);
        self.xp(75, "Krabban fångad");
        self.refresh_markers();
        self.place_crab();
    }

    pub fn animal_event(&mut self, name: &str) {
        self.stat("animals");
        let t = animal_text(name);
        self.toast(t, ToastKind::Info);
    }

    // ------------------------------------------------------------------ svårt läge: bevis

    /// Anropas för varje inskriven rad: noterar beviskommandon för olösta uppgifter
    pub fn note_proof_command(&mut self, dev: &str, line: &str) {
        if !self.hard() {
            return;
        }
        let (verify, matches) = (self.verify_fn, self.matches_fn);
        let run = match &mut self.run {
            Some(r) => r,
            None => return,
        };
        let w = match &run.week {
            Some(w) => w,
            None => return,
        };
        for (i, t) in w.tasks.iter().enumerate() {
            if run.ts[i].fixed || run.x.pending_proof.contains(&i) {
                continue;
            }
            let v = verify(t.id);
            if matches(&v, dev, line) {
                run.x.pending_proof.push(i);
            }
        }
    }

    /// Efter ett kommando: för varje väntande bevis körs check() direkt. Är felet rättat bekräftas det.
    pub fn settle_proofs(&mut self) {
        let pend = match &mut self.run {
            Some(r) => std::mem::take(&mut r.x.pending_proof),
            None => return,
        };
        if pend.is_empty() {
            return;
        }
        for i in pend {
            let ok = {
                let run = self.run.as_mut().unwrap();
                if run.ts[i].fixed || run.done {
                    false
                } else {
                    match run.week.as_ref() {
                        Some(w) => (w.tasks[i].check)(&mut run.st),
                        None => false,
                    }
                }
            };
            if ok {
                self.confirm_task(i, true);
            }
        }
    }

    /// Beviskravet i ärendelistan (svårt läge)
    pub fn proof_text(&self, task_id: &str) -> String {
        (self.verify_fn)(task_id).text.to_string()
    }

    // ------------------------------------------------------------------ kartan

    pub fn open_map(&mut self) {
        let run = match &self.run {
            Some(r) => r,
            None => return,
        };
        let mut sites = vec![("gbg".to_string(), "Göteborg".to_string()), ("boras".to_string(), "Borås".to_string())];
        if run.st.devices.contains_key("RM") {
            sites.push(("molndal".to_string(), "Mölndal".to_string()));
        }
        let mv = MapView::new(sites, &run.world.site, self.hard());
        self.overlays.push(Overlay::Map(mv));
    }

    /// Kartans val: snabbresa eller klick på kartan (gå dit, efter en snabbresa om platsen byts)
    pub fn map_act(&mut self, act: MapAct, rect: super::ui_widgets::Rect) {
        match act {
            MapAct::None | MapAct::Close => {}
            MapAct::Travel(site) => {
                self.close_map();
                self.travel_pub(&site);
                self.stat("trips");
            }
            MapAct::Walk(site, px, py) => {
                self.close_map();
                let here = self.run.as_ref().map(|r| r.world.site.clone()).unwrap_or_default();
                if here != site {
                    self.travel_pub(&site);
                    self.stat("trips");
                }
                if let Some(run) = &mut self.run {
                    let (wx, wz) = run.world.map_to_world(&site, rect.x + 4, rect.y + 4, rect.w - 8, rect.h - 8, px, py);
                    run.world.walk_to_point(wx, wz);
                }
            }
        }
    }

    fn close_map(&mut self) {
        if matches!(self.overlays.last(), Some(Overlay::Map(_))) {
            self.overlays.pop();
        }
    }
}

#[allow(unused)]
fn _use() {
    let _ = S::refresh;
}

// ------------------------------------------------------------------ tester

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::levels::Verify;
    use crate::game::ui_save::SaveData;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static FIXED: AtomicBool = AtomicBool::new(false);
    // testerna delar fil och miljövariabel: kör dem efter varandra
    static LOCK: Mutex<()> = Mutex::new(());

    fn fake_verify(_id: &str) -> Verify {
        Verify { on: vec!["SW1"], cmds: vec!["ping"], text: "Bevisa: ping från SW1." }
    }
    fn fake_matches(v: &Verify, dev: &str, line: &str) -> bool {
        v.on.contains(&dev) && v.cmds.iter().any(|c| line.trim().to_lowercase().starts_with(c))
    }
    fn fake_check(_: &mut crate::net::sim::State) -> bool {
        FIXED.load(Ordering::SeqCst)
    }

    fn app(difficulty: &str) -> App {
        unsafe {
            std::env::set_var("KRABBA_SAVE", std::env::temp_dir().join("krabba-test-save.json"));
        }
        let mut a = App::new();
        a.save = SaveData::default();
        a.save.settings.difficulty = difficulty.into();
        a.verify_fn = fake_verify;
        a.matches_fn = fake_matches;
        a.start_week(1, false);
        a.overlays.clear();
        // bara första uppgiften, med en fejkad kontroll
        let run = a.run.as_mut().unwrap();
        let w = run.week.as_mut().unwrap();
        w.tasks.truncate(1);
        w.tasks[0].check = fake_check;
        run.ts.truncate(1);
        FIXED.store(false, Ordering::SeqCst);
        a
    }

    fn toast_has(a: &App, s: &str) -> bool {
        a.toasts.iter().any(|t| t.text.contains(s))
    }

    #[test]
    fn svart_lage_kraver_bevis() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("hard");
        // beviset innan felet är rättat: inget händer
        a.on_command("SW1", "ping 10.0.0.1");
        a.after_command();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed);
        // felet rättas men spelet säger inget
        FIXED.store(true, Ordering::SeqCst);
        a.toasts.clear();
        a.check_tasks();
        a.housekeeping_for_test();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed, "får inte bekräftas utan bevis");
        assert!(a.toasts.iter().all(|t| !t.text.contains("löst")), "ingen notis om löst fel");
        assert!(a.pending_reports().is_empty());
        // fel enhet räknas inte
        a.on_command("PC-Anna", "ping 10.0.0.1");
        a.after_command();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed);
        // fel kommando räknas inte
        a.on_command("SW1", "show version");
        a.after_command();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed);
        // rätt bevis
        let xp0 = a.save.career.xp;
        a.on_command("SW1", "ping 10.0.0.1");
        a.after_command();
        assert!(a.run.as_ref().unwrap().ts[0].fixed);
        assert!(toast_has(&a, "Bevisat!"));
        // 150 XP (utan ledtråd) ×1.5 = 225 (utöver bonusar för nya kommandon)
        assert!(a.save.career.xp - xp0 >= 225);
        assert_eq!(a.pending_reports(), vec![0]);
        // ingen påminnelse i svårt läge
        assert!(a.delayed.is_empty());
    }

    #[test]
    fn beviset_maste_lyckas_direkt() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("hard");
        FIXED.store(true, Ordering::SeqCst);
        // kommandot skrivs medan felet finns kvar...
        a.on_command("SW1", "ping 10.0.0.1");
        FIXED.store(false, Ordering::SeqCst);
        a.after_command();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed);
        // ...och räknas inte senare heller
        FIXED.store(true, Ordering::SeqCst);
        a.after_command();
        assert!(!a.run.as_ref().unwrap().ts[0].fixed);
    }

    #[test]
    fn normalt_lage_bockar_direkt() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("normal");
        FIXED.store(true, Ordering::SeqCst);
        a.check_tasks();
        assert!(a.run.as_ref().unwrap().ts[0].fixed);
        assert!(toast_has(&a, "är löst!"));
        assert_eq!(a.pending_reports(), vec![0]);
        assert!(a.save.career.xp >= 150);
    }

    #[test]
    fn xp_skalas_efter_svarighet() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for (d, want) in [("easy", 75), ("normal", 100), ("hard", 150)] {
            let mut a = app(d);
            a.xp(100, "");
            assert_eq!(a.save.career.xp, want, "{}", d);
        }
        let mut a = app("hard");
        a.xp(-15, "");
        assert_eq!(a.save.career.xp, 0);
    }

    #[test]
    fn karta_flik_och_karriar() {
        use crate::engine::{Input, Key};
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("normal");
        let key = |k: Key| {
            let mut i = Input::default();
            i.pressed.push(k);
            i
        };
        a.open_map();
        a.update_overlays(&Input::default());
        a.update_overlays(&key(Key::Num2));
        assert_eq!(a.overlays.len(), 1, "kartan ska stanna öppen");
        match &a.overlays[0] {
            Overlay::Map(m) => assert_eq!(m.site, "boras"),
            _ => panic!(),
        }
        a.update_overlays(&key(Key::Escape));
        assert!(a.overlays.is_empty());
        a.update_play(&key(Key::J), 0.016);
        assert!(matches!(a.overlays.last(), Some(Overlay::Career(_))));
    }

    #[test]
    fn kaffe_och_djur() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("normal");
        a.drink_coffee();
        assert_eq!(a.run.as_ref().unwrap().world.speed_mult, COFFEE_MULT);
        assert_eq!(a.save.career.stats["coffees"], 1.0);
        a.run.as_mut().unwrap().elapsed += COFFEE_SECS + 1.0;
        a.career_tick();
        assert_eq!(a.run.as_ref().unwrap().world.speed_mult, 1.0);
        a.animal_event("Katten");
        assert_eq!(a.save.career.stats["animals"], 1.0);
        assert!(animal_text("Ankorna").contains("Kvack"));
    }

    #[test]
    fn utmaning_sparas() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = app("normal");
        // vecka 1: utmaningen är att inte köra show running-config
        assert_eq!(a.challenge_status().map(|s| s.1), Some(true));
        a.on_command("SW1", "show running-config");
        assert_eq!(a.challenge_status().map(|s| s.1), Some(false));
        let mut b = app("normal");
        b.mark_challenge(1);
        assert!(b.challenge_marked(1));
        assert_eq!(b.save.career.stats["challenges"], 1.0);
        b.mark_challenge(1);
        assert_eq!(b.save.career.stats["challenges"], 1.0);
    }
}

#[cfg(test)]
impl App {
    fn housekeeping_for_test(&mut self) {
        self.check_tasks();
        self.career_tick();
    }
}

#[cfg(test)]
mod sound_tests {
    use super::*;
    use crate::engine::{audio, Input, Key};
    use crate::engine::Game;
    use crate::game::ui_save::SaveData;

    fn peak() -> f32 {
        let mut buf = vec![0f32; 2 * 4410];
        audio::render(&mut buf);
        buf.iter().fold(0f32, |a, b| a.max(b.abs()))
    }

    #[test]
    fn spelet_ger_ljud_och_tystnar() {
        unsafe {
            std::env::set_var("KRABBA_SAVE", std::env::temp_dir().join("krabba-test-save-ljud.json"));
        }
        let mut a = App::new();
        a.save = SaveData::default();
        audio::unlock();
        audio::set_settings(a.save.settings.audio());
        a.start_week(1, false);
        a.overlays.clear();
        peak();
        // gå ett tag: fotsteg och rumston
        let mut best = 0f32;
        for _ in 0..120 {
            let mut i = Input::default();
            i.down.insert(Key::D);
            a.update(&i, 1.0 / 60.0);
            best = best.max(peak());
        }
        assert!(best > 0.001, "fotsteg ger ljud: {best}");
        // öppna terminalen och skriv
        let mut t = Input::default();
        t.pressed.push(Key::T);
        a.update(&t, 1.0 / 60.0);
        let mut best = peak();
        for c in "help".chars() {
            let mut i = Input::default();
            i.chars.push(c);
            a.update(&i, 1.0 / 60.0);
            best = best.max(peak());
        }
        assert!(best > 0.001, "terminal ger ljud: {best}");
        // ljud av: tyst
        a.save.settings.sound = false;
        for _ in 0..30 {
            let mut i = Input::default();
            i.down.insert(Key::A);
            a.update(&i, 1.0 / 60.0);
        }
        audio::set_settings(a.save.settings.audio());
        assert_eq!(peak(), 0.0);
        audio::set_settings(crate::game::ui_save::Settings::default().audio());
    }

    #[test]
    fn inställningar_sparas_och_tolkas() {
        let s: crate::game::ui_save::Settings = serde_json::from_str("{\"difficulty\":\"hard\"}").unwrap();
        assert!(s.sound && !s.music && s.type_sound);
        assert!((s.volume - 0.6).abs() < 1e-6 && s.fx == 1.0 && s.ambience == 1.0);
        let a = s.audio();
        assert!(a.sound && (a.volume - 0.6).abs() < 1e-6);
    }
}
