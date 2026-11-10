//! Lb-sessionen. Port av js/cli/lb.js (och LB-tilläggen i js/cli/more7.js).
use super::{Cli, CmdResult};
use crate::net::model::Lb;
use crate::net::sim::{self, LbPoolStatus, PingOpts, State};
use crate::net::util as U;
use indexmap::IndexMap;

const HELP: &str = "show pool [namn]                      servrarna i poolen och hälsokontrollens svar\nshow stats                            förfrågningar och fel per server\nshow monitor                          vilken hälsokontroll poolen använder\nshow running-config                   hela konfigurationen\nconfig pool <namn> monitor <icmp|tcp|http>\nconfig pool <namn> member <server> <enable|disable>\nclear stats                           nollställ räknarna\nsave config                           spara (överlever omstart)\nlogout                                avsluta";
const HELP_EXTRA: &str = "\nshow version             version och licens\nshow interfaces          lastbalanserarens egen adress\nping <ip>                testa om en server svarar";
const COMPLETE: [&str; 9] = [
    "show pool",
    "show stats",
    "show monitor",
    "show running-config",
    "config pool TID-POOL monitor http",
    "config pool TID-POOL member",
    "clear stats",
    "save config",
    "logout",
];

/// Väntande fråga (JS: this.pending).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    Login,
    Password,
    Unsaved,
}

fn ask_text(a: Ask) -> &'static str {
    match a {
        Ask::Login => "login: ",
        Ask::Password => "Password: ",
        Ask::Unsaved => "Det finns osparade ändringar. Spara nu? (y/N) ",
    }
}

/// JS MON-tabellen: hälsokontrollens beskrivning.
fn mon_text(t: &str) -> Option<&'static str> {
    match t {
        "icmp" => Some("icmp (ping mot servern)"),
        "tcp" => Some("tcp (anslutning till porten)"),
        "http" => Some("http (GET /status, väntar 200 OK)"),
        _ => None,
    }
}

pub struct LbSession {
    pub dev: String,
    pub closed: bool,
    pending: Option<Ask>,
}

impl LbSession {
    pub fn new(st: &State, dev: &str) -> LbSession {
        let _ = st;
        LbSession { dev: dev.to_string(), closed: false, pending: Some(Ask::Login) }
    }
    fn lb<'a>(&self, st: &'a State) -> &'a Lb {
        st.dev(&self.dev).lb.as_ref().expect("LB-enhet")
    }
    fn lb_mut<'a>(&self, st: &'a mut State) -> &'a mut Lb {
        st.dev_mut(&self.dev).lb.as_mut().expect("LB-enhet")
    }
    fn close(&mut self) -> CmdResult {
        self.closed = true;
        CmdResult { close: true, ..Default::default() }
    }
    fn answer(&mut self, st: &mut State, p: Ask, x: &str) -> CmdResult {
        match p {
            Ask::Login => {
                self.pending = Some(Ask::Password);
                CmdResult::out("")
            }
            Ask::Password => {
                CmdResult::out("\nLB-Nordvik 4.2 (lastbalanserare för tidrapporteringen)\nSkriv help för kommandon.\n")
            }
            Ask::Unsaved => {
                if x.to_lowercase().starts_with('y') {
                    let lb = self.lb_mut(st);
                    lb.saved = Some(lb.pools.clone());
                }
                self.close()
            }
        }
    }

    fn show(&self, st: &mut State, a: &[&str]) -> String {
        let w = a.get(1).copied().unwrap_or("").to_lowercase();
        if m(&w, "pool", 1) {
            return self.show_pool(st, a.get(2).copied());
        }
        if m(&w, "stats", 2) {
            return self.show_stats(st);
        }
        if m(&w, "monitor", 1) {
            return self.show_monitor(st);
        }
        if m(&w, "running-config", 1) {
            return self.running(st);
        }
        "% Okänt show-kommando. Prova: show pool, show stats, show monitor".into()
    }

    fn show_pool(&self, st: &mut State, name: Option<&str>) -> String {
        let list = sim::lb_status(st, &self.dev);
        let rows: Vec<&LbPoolStatus> = list
            .iter()
            .filter(|p| name.map_or(true, |n| p.name.to_lowercase() == n.to_lowercase()))
            .collect();
        if rows.is_empty() {
            return "% Poolen finns inte.".into();
        }
        let mut o: Vec<String> = vec![];
        for p in rows {
            o.push(format!(
                "Pool {}   VIP {}:{}   metod {}   hälsokontroll {}",
                p.name, p.pool.vip, p.pool.port, p.pool.method, p.pool.monitor
            ));
            o.push("  Server    Adress              Admin     Hälsa   Förfr.   Fel".into());
            o.push("  --------  ------------------  --------  ------  -------  -----".into());
            for x in &p.members {
                let health = if !x.m.enabled { "-" } else if x.up { "UP" } else { "DOWN" };
                o.push(format!(
                    "  {}{}{}{}{}{}",
                    U::pad(&x.m.name, 10),
                    U::pad(format!("{}:{}", x.m.ip, x.m.port), 20),
                    U::pad(if x.m.enabled { "enabled" } else { "disabled" }, 10),
                    U::pad(health, 8),
                    U::pad_l(x.req, 7),
                    U::pad_l(x.fail, 7),
                ));
            }
            o.push(String::new());
        }
        o.join("\n")
    }

    fn show_stats(&self, st: &mut State) -> String {
        let list = sim::lb_status(st, &self.dev);
        let mut o: Vec<String> = vec!["Server    Förfrågningar  Fel   Felandel".into()];
        for p in &list {
            for x in &p.members {
                // Math.round på positiva tal: halva uppåt
                let pct = if x.req != 0 { format!("{} %", (x.fail * 200 + x.req) / (2 * x.req)) } else { "–".into() };
                o.push(format!(
                    "{}{}{}{}",
                    U::pad(&x.m.name, 10),
                    U::pad_l(x.req, 13),
                    U::pad_l(x.fail, 5),
                    U::pad_l(pct, 11)
                ));
            }
        }
        o.join("\n")
    }

    fn show_monitor(&self, st: &State) -> String {
        self.lb(st)
            .pools
            .iter()
            .map(|(n, p)| {
                format!("Pool {}: {}, var 5:e sekund, 3 missar = DOWN", n, mon_text(&p.monitor).unwrap_or("undefined"))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn running(&self, st: &State) -> String {
        let lb = self.lb(st);
        let mut o: Vec<String> = vec![
            "! LB-Nordvik".into(),
            format!("hostname {}", lb.name),
            "interface eth0 address 192.168.1.13/26 gateway 192.168.1.1".into(),
            "!".into(),
        ];
        for (n, p) in &lb.pools {
            o.push(format!("pool {}", n));
            o.push(format!(" vip {} port {}", p.vip, p.port));
            o.push(format!(" method {}", p.method));
            o.push(format!(
                " monitor {}{}",
                p.monitor,
                if p.monitor == "http" { " path /status expect 200" } else { "" }
            ));
            for mem in &p.members {
                o.push(format!(" member {} {}:{}{}", mem.name, mem.ip, mem.port, if mem.enabled { "" } else { " disabled" }));
            }
            o.push("!".into());
        }
        o.join("\n")
    }

    fn config(&self, st: &mut State, a: &[&str]) -> String {
        let a1 = a.get(1).copied().unwrap_or("");
        if !m(a1, "pool", 1) {
            return "% Använd: config pool <namn> monitor <icmp|tcp|http>".into();
        }
        let want = a.get(2).copied().unwrap_or("").to_lowercase();
        let pname: Option<String> = self.lb(st).pools.keys().find(|n| n.to_lowercase() == want).cloned();
        let Some(pname) = pname else {
            let names: Vec<&str> = self.lb(st).pools.keys().map(|k| k.as_str()).collect();
            return format!("% Poolen \"{}\" finns inte. Pooler: {}", a.get(2).copied().unwrap_or(""), names.join(", "));
        };
        let a3 = a.get(3).copied().unwrap_or("");
        if m(a3, "monitor", 1) {
            let t = a.get(4).copied().unwrap_or("").to_lowercase();
            let Some(text) = mon_text(&t) else {
                return "% Hälsokontroll måste vara icmp, tcp eller http.".into();
            };
            self.lb_mut(st).pools.get_mut(&pname).expect("pool").monitor = t;
            sim::touch(st);
            return format!("Pool {}: hälsokontroll {}.", pname, text);
        }
        if m(a3, "member", 1) {
            let raw = a.get(4).copied().unwrap_or("");
            let lower = raw.to_lowercase();
            let found = self
                .lb(st)
                .pools
                .get(&pname)
                .expect("pool")
                .members
                .iter()
                .find(|x| x.name == lower || x.ip == raw)
                .map(|x| x.name.clone());
            let Some(mname) = found else {
                return format!("% Servern \"{}\" finns inte i {}.", raw, pname);
            };
            let op = a.get(5).copied().unwrap_or("").to_lowercase();
            if op != "enable" && op != "disable" {
                return format!("% Använd: config pool {} member {} <enable|disable>", pname, mname);
            }
            let en = op == "enable";
            let pool = self.lb_mut(st).pools.get_mut(&pname).expect("pool");
            if let Some(mem) = pool.members.iter_mut().find(|x| x.name == mname) {
                mem.enabled = en;
            }
            sim::touch(st);
            return format!(
                "Server {} {}.",
                mname,
                if en { "aktiverad" } else { "avaktiverad (får ingen trafik)" }
            );
        }
        "% Använd: config pool <namn> monitor <icmp|tcp|http>  |  config pool <namn> member <server> <enable|disable>".into()
    }

    fn show_interfaces(&self, st: &mut State) -> String {
        let d = sim::get(st);
        let eps = sim::dev_eps(&d, &self.dev);
        if eps.is_empty() {
            return "Interface  Adress            Status\neth0       –                 down".into();
        }
        let mut o: Vec<String> = vec!["Interface  Adress            Status".into()];
        for e in eps {
            let name = if e.iface == "nic" { "eth0" } else { e.iface.as_str() };
            let prefix = e
                .mask
                .as_deref()
                .and_then(U::mask_to_prefix)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "null".into());
            o.push(format!(
                "{}{}{}",
                U::pad(name, 11),
                U::pad(format!("{}/{}", e.ip.as_deref().unwrap_or("null"), prefix), 18),
                if e.up { "up" } else { "down" }
            ));
        }
        o.join("\n")
    }
}

impl Cli for LbSession {
    fn prompt_text(&self, st: &State) -> String {
        match self.pending {
            Some(p) => ask_text(p).into(),
            None => format!("{}# ", self.lb(st).name),
        }
    }

    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        if let Some(p) = self.pending.take() {
            return self.answer(st, p, line.trim());
        }
        let a: Vec<&str> = line.split_whitespace().collect();
        if a.is_empty() {
            return CmdResult::out("");
        }
        let c0 = a[0].to_lowercase();
        let a1 = a.get(1).copied().unwrap_or("").to_lowercase();

        // Tilläggen från more7.js
        if c0 == "show" && a1.starts_with("ver") {
            return CmdResult::out("LB-Nordvik 4.2.1 (build 2026-03-11)\nModell: vLB-200, 2 vCPU, 4 GB\nUpptid: 41 dagar, 3 timmar\nLicens: 200 Mbit/s");
        }
        if c0 == "show" && a1.starts_with("int") {
            return CmdResult::out(self.show_interfaces(st));
        }
        if c0 == "ping" {
            if let Some(&dst) = a.get(1) {
                let p = sim::ping(st, &self.dev, dst, PingOpts::default());
                let out = if p.ok {
                    format!("{} svarar ({} ms)", dst, 1 + p.hops.len())
                } else {
                    format!("{} svarar inte", dst)
                };
                return CmdResult { out, delay: if p.ok { 400 } else { 2000 }, ..Default::default() };
            }
        }

        if c0 == "?" || m(&c0, "help", 2) {
            let extra = c0 == "?" || c0 == "help";
            return CmdResult::out(if extra { format!("{}{}", HELP, HELP_EXTRA) } else { HELP.to_string() });
        }
        if m(&c0, "logout", 2) || m(&c0, "exit", 2) || m(&c0, "quit", 2) {
            let lb = self.lb(st);
            let changed = match &lb.saved {
                None => true,
                Some(s) => !s.iter().eq(lb.pools.iter()),
            };
            if changed {
                self.pending = Some(Ask::Unsaved);
                return CmdResult::out("");
            }
            return self.close();
        }
        if m(&c0, "save", 2) && m(a1.as_str(), "config", 2) {
            let lb = self.lb_mut(st);
            lb.saved = Some(lb.pools.clone());
            return CmdResult::out("Konfigurationen sparad.");
        }
        if m(&c0, "clear", 2) && m(a1.as_str(), "stats", 2) {
            self.lb_mut(st).stats = IndexMap::new();
            return CmdResult::out("Räknarna nollställda.");
        }
        if m(&c0, "show", 2) {
            return CmdResult::out(self.show(st, &a));
        }
        if m(&c0, "config", 1) {
            return CmdResult::out(self.config(st, &a));
        }
        CmdResult::out(format!("% Okänt kommando \"{}\". Skriv help.", a[0]))
    }

    fn help(&mut self, st: &mut State, line: &str) -> String {
        let _ = (st, line);
        format!("{}{}", HELP, HELP_EXTRA)
    }

    fn complete(&mut self, st: &mut State, line: &str) -> Option<String> {
        let _ = st;
        let t = line.trim();
        if t.is_empty() {
            return None;
        }
        let hit: Vec<&str> = COMPLETE.iter().copied().filter(|w| w.starts_with(t)).collect();
        if hit.len() == 1 { Some(format!("{} ", hit[0])) } else { None }
    }

    fn secret_input(&self) -> bool {
        self.pending == Some(Ask::Password)
    }

    fn closed(&self) -> bool {
        self.closed
    }

    fn dev_id(&self) -> &str {
        &self.dev
    }
}

/// JS m(word, full, min): ordet (gemener) är ett prefix av kommandot och minst `min` tecken.
fn m(word: &str, full: &str, min: usize) -> bool {
    word.chars().count() >= min.max(1) && full.starts_with(&word.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::model;

    fn setup() -> (State, LbSession) {
        let st = State::new(model::build_golden(10));
        let s = LbSession::new(&st, "LB");
        (st, s)
    }

    fn login(st: &mut State, s: &mut LbSession) {
        s.handle(st, "admin");
        s.handle(st, "geheim");
    }

    #[test]
    fn inloggning() {
        let (mut st, mut s) = setup();
        assert_eq!(s.prompt_text(&st), "login: ");
        s.handle(&mut st, "admin");
        assert_eq!(s.prompt_text(&st), "Password: ");
        assert!(s.secret_input());
        let r = s.handle(&mut st, "geheim");
        assert!(r.out.contains("LB-Nordvik 4.2"));
        assert_eq!(s.prompt_text(&st), "LB-Nordvik# ");
    }

    #[test]
    fn monitor_och_medlem_andras() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        let r = s.handle(&mut st, "config pool TID-POOL monitor tcp");
        assert_eq!(r.out, "Pool TID-POOL: hälsokontroll tcp (anslutning till porten).");
        assert_eq!(st.dev("LB").lb.as_ref().unwrap().pools["TID-POOL"].monitor, "tcp");
        let r = s.handle(&mut st, "config pool TID-POOL member tid-2 disable");
        assert_eq!(r.out, "Server tid-2 avaktiverad (får ingen trafik).");
        let pool = &st.dev("LB").lb.as_ref().unwrap().pools["TID-POOL"];
        assert!(!pool.members[1].enabled);
        assert!(s.handle(&mut st, "config pool TID-POOL monitor ftp").out.contains("Hälsokontroll måste vara"));
        assert!(s.handle(&mut st, "config pool FINNS-EJ monitor tcp").out.contains("finns inte. Pooler: TID-POOL"));
        assert!(s.handle(&mut st, "config pool TID-POOL member tid-9 enable").out.contains("finns inte i TID-POOL"));
    }

    #[test]
    fn spara_och_logout() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        s.handle(&mut st, "config pool TID-POOL monitor icmp");
        assert_eq!(s.handle(&mut st, "logout").out, "");
        assert_eq!(s.prompt_text(&st), "Det finns osparade ändringar. Spara nu? (y/N) ");
        // Som i JS stängs sessionen oavsett svar, men bara "y" sparar
        assert!(s.handle(&mut st, "n").close);
        assert!(s.closed());
        assert_eq!(st.dev("LB").lb.as_ref().unwrap().saved.as_ref().unwrap()["TID-POOL"].monitor, "http");

        let mut s2 = LbSession::new(&st, "LB");
        login(&mut st, &mut s2);
        assert_eq!(s2.handle(&mut st, "save config").out, "Konfigurationen sparad.");
        assert_eq!(st.dev("LB").lb.as_ref().unwrap().saved.as_ref().unwrap()["TID-POOL"].monitor, "icmp");
        assert!(s2.handle(&mut st, "exit").close);
    }

    #[test]
    fn hjalp_och_okant() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        let h = s.handle(&mut st, "help").out;
        assert!(h.contains("show version"));
        assert!(h.ends_with("testa om en server svarar"));
        assert_eq!(s.handle(&mut st, "xyz").out, "% Okänt kommando \"xyz\". Skriv help.");
        assert_eq!(s.handle(&mut st, "show foo").out, "% Okänt show-kommando. Prova: show pool, show stats, show monitor");
    }

    #[test]
    fn running_config_visar_pool() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        let r = s.handle(&mut st, "show running-config").out;
        assert!(r.starts_with("! LB-Nordvik\nhostname LB-Nordvik\n"));
        assert!(r.contains("pool TID-POOL\n vip 192.168.1.13 port 80\n method round-robin\n monitor http path /status expect 200"));
        assert!(r.contains(" member tid-1 192.168.1.14:80\n"));
    }

    #[test]
    fn komplettering() {
        let (st, mut s) = setup();
        let _ = st;
        assert_eq!(s.complete(&mut State::new(model::build_golden(10)), "lo"), Some("logout ".into()));
    }
}
