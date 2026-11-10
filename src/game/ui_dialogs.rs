//! Spelets dialoger: briefing, ärenden, ledtrådar, felrapport, datorer, kablar, mobil, frågesport, inställningar m.m.
//! Port av js/ui/ui.js, more.js och v7/v8 (de delar som behövs för spelet).
use super::handbook_data::{self as data, Block};
use super::ui_dialog::*;
use super::{App, Overlay, QItem, QuizRun, ReportData, ToastKind};
use crate::net::model::{Kind, StaticIp};
use crate::net::sim::{self as S, PingOpts};
use crate::net::util as U;

/// `Namn: text` -> `text` (samma som ticket.replace(/^[^:]+:\s*/, '') i JS)
fn strip_speaker(s: &str) -> String {
    match s.find(':') {
        Some(p) => s[p + 1..].trim_start().to_string(),
        None => s.to_string(),
    }
}

/// Blandad ordning efter hash (som shuffled() i ui.js): original-index i visningsordning
pub fn shuffled(arr: &[&str], seed: &str) -> Vec<usize> {
    let mut v: Vec<(u32, usize)> = arr.iter().enumerate().map(|(i, x)| (U::hash(&format!("{}{}", seed, x)), i)).collect();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| x.1).collect()
}

pub fn blocks_to_items(blocks: &[Block]) -> Vec<Item> {
    let mut v = vec![];
    for b in blocks {
        match b {
            Block::H(s) => v.push(Item::Text(s.to_string(), Tone::Head)),
            Block::P(s) => v.push(Item::Text(s.to_string(), Tone::Muted)),
            Block::T(rows) => v.push(Item::Table(rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect())),
            Block::L(items) => {
                for i in items.iter() {
                    v.push(Item::Text(format!("• {}", i), Tone::Normal));
                }
            }
        }
    }
    v
}

/// Nivåer för volymreglagen
pub const LEVELS: [f32; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

fn level_idx(v: f32) -> usize {
    LEVELS.iter().enumerate().min_by(|a, b| (a.1 - v).abs().partial_cmp(&(b.1 - v).abs()).unwrap()).map(|x| x.0).unwrap_or(6)
}

impl App {
    // ------------------------------------------------------------------ briefing och paus

    pub fn briefing_dialog(&self, first: bool) -> Dialog {
        let run = self.run.as_ref();
        let mut d = Dialog::new(if first { "Krabba spelet" } else { "Tavlan" }).wide();
        match run.and_then(|r| r.week.as_ref()) {
            Some(w) => {
                d = d.text(w.intro);
                let c = match w.course {
                    Some(c) => format!("{}. ", c),
                    None => format!("Kapitel {} i kursboken. ", w.chapter),
                };
                d = d.tone(format!("{}{}", c, if w.build.is_some() { "Allt görs i CLI:t – varje steg bockas av så fort nätet fungerar. Du tränar på:" } else { "Den här veckan tränar du på:" }), Tone::Muted);
                for l in &w.learn {
                    d = d.text(format!("• {}", l));
                }
                if let Some(ch) = w.challenge {
                    let done = run.map(|r| self.challenge_marked(r.n)).unwrap_or(false);
                    d = d.tone(format!("Veckans utmaning: {}{}", ch, if done { " (redan klarad)" } else { " – ger 75 XP" }), Tone::Muted);
                }
                if self.hard() {
                    d = d.tone("SVÅRT LÄGE: spelet säger inte till när ett fel är löst. Bevisa det med ett kommando (kravet står i ärendelistan, Tab), skriv sedan felrapporten.", Tone::Bad);
                }
            }
            None => d = d.text("Fri träning: Nordviks nät utan fel. Koppla in dig var du vill, bygg om och testa."),
        }
        if run.map(|r| r.exam).unwrap_or(false) {
            d = d.tone("EXAMENSLÄGE: inga ledtrådar och ingen guide.", Tone::Bad);
        }
        d = d.tone("Konsolen kräver inget lösenord. SSH-kontot står på lappen vid laptopen: drift / nordvik. Pilen/kompassen visar vägen till nästa mål.", Tone::Muted);
        d.btn(Btn::primary(if first { "Sätt igång" } else { "Stäng" }, Act::Close))
    }

    pub fn pause_dialog(&self) -> Dialog {
        Dialog::new("Paus")
            .text("Spelet sparas automatiskt varje tiotal sekunder och när du går till menyn.")
            .btn(Btn::primary("Fortsätt", Act::Close))
            .btn(Btn::new("Ärenden", Act::Tasks))
            .btn(Btn::new("Handbok", Act::Handbook))
            .btn(Btn::new("Inställningar", Act::SettingsOpen))
            .btn(Btn::new("Till menyn", Act::Menu))
    }

    // ------------------------------------------------------------------ kollegor

    pub fn npc_dialog(&mut self, name: &str) -> Dialog {
        let mut d = Dialog::new(name);
        let week_n = self.run.as_ref().map(|r| r.n).unwrap_or(0);
        let mut unfixed: Option<usize> = None;
        let exam = self.run.as_ref().map(|r| r.exam).unwrap_or(false);
        let mut any = false;
        if let Some(run) = &mut self.run {
            if let Some(w) = run.week.clone() {
                for (i, t) in w.tasks.iter().enumerate() {
                    if t.npc != name {
                        continue;
                    }
                    any = true;
                    let s = &mut run.ts[i];
                    if !s.known {
                        run.last_progress = run.elapsed;
                    }
                    s.known = true;
                    let line = if s.reported {
                        "\"Tack! Nu fungerar det igen.\"".to_string()
                    } else if s.fixed {
                        "\"Det verkar fungera nu! Glöm inte att skriva felrapporten.\"".to_string()
                    } else {
                        if unfixed.is_none() {
                            unfixed = Some(i);
                        }
                        strip_speaker(t.ticket)
                    };
                    d = d.tone(line, Tone::Quote);
                }
            }
        }
        if !any {
            let pick = |list: &'static [(&'static str, &'static [&'static str])]| list.iter().find(|(n, _)| *n == name).map(|(_, l)| *l);
            let lines: &[&str] = if week_n == 10 { pick(data::CHAT10).or_else(|| pick(data::CHAT)) } else { pick(data::CHAT) }.unwrap_or(&["\"Allt verkar lugnt här i dag.\""]);
            let t = self.run.as_ref().map(|r| r.st.time).unwrap_or(0.0);
            let ix = (U::hash(name) as usize + week_n as usize + (t / 60.0) as usize) % lines.len();
            d = d.tone(lines[ix], Tone::Quote);
        }
        self.refresh_markers();
        if let Some(i) = unfixed {
            if !exam {
                d = d.btn(Btn::new("Ledtråd", Act::OpenHint(i)));
            }
        }
        d.btn(Btn::primary("Okej, jag kollar", Act::Close))
    }

    // ------------------------------------------------------------------ datorer och kablar

    pub fn pc_dialog(&self, id: &str) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let h = match run.st.devices.get(id) {
            Some(h) => h,
            None => return Dialog::new("Dator").text("Datorn finns inte.").btn(Btn::primary("Stäng", Act::Close)),
        };
        let c = S::host_ip_conf(h);
        let label = if h.label.is_empty() { id.to_string() } else { h.label.clone() };
        let dhcp = h.nic.as_ref().map(|n| n.dhcp).unwrap_or(true);
        let ip = c.and_then(|c| c.ip).unwrap_or_else(|| "–".into());
        let mut d = Dialog::new("Dator").text(label).tone(format!("IPv4: {} ({})", ip, if dhcp { "DHCP" } else { "statisk" }), Tone::Muted);
        d = d.btn(Btn::primary("Kommandotolken", Act::OpenWin(id.to_string()))).btn(Btn::new("Nätverksinställningar", Act::OpenIpv4(id.to_string())));
        if id != "Gast-laptop" {
            d = d.btn(Btn::new("Byt patchkabel", Act::ReplaceCable(id.to_string())));
        }
        d.btn(Btn::new("Stäng", Act::Close))
    }

    pub fn ipv4_dialog(&self, id: &str) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let h = &run.st.devices[id];
        let nic = h.nic.clone();
        let st = nic.as_ref().and_then(|n| n.static_.clone());
        let c = S::host_ip_conf(h);
        let dhcp = nic.as_ref().map(|n| n.dhcp).unwrap_or(true);
        let ip = st.as_ref().map(|s| s.ip.clone()).or_else(|| c.as_ref().and_then(|c| c.ip.clone())).unwrap_or_default();
        let mask = st.as_ref().map(|s| s.mask.clone()).or_else(|| c.as_ref().and_then(|c| c.mask.clone())).unwrap_or_else(|| "255.255.255.192".into());
        let gw = st.as_ref().and_then(|s| s.gw.clone()).or_else(|| c.as_ref().and_then(|c| c.gw.clone())).unwrap_or_default();
        let dns = st.as_ref().and_then(|s| s.dns.first().cloned()).or_else(|| c.as_ref().and_then(|c| c.dns.first().cloned())).unwrap_or_default();
        let inp = |id: &'static str, label: &str, v: String| {
            let n = v.chars().count();
            Item::Input { id, label: label.into(), value: v, rows: 1, hint: String::new(), cur: n }
        };
        let label = if h.label.is_empty() { id.to_string() } else { h.label.clone() };
        let mut d = Dialog::new(&format!("Nätverksinställningar – {}", label));
        d = d.tone("Egenskaper för Internet Protocol version 4 (TCP/IPv4)", Tone::Muted);
        d = d.item(Item::Radio { id: "m", legend: "Adressering".into(), options: vec!["Erhåll en IP-adress automatiskt".into(), "Använd följande IP-adress:".into()], sel: Some(if dhcp { 0 } else { 1 }) });
        d = d.item(inp("ip", "IP-adress:", ip)).item(inp("mask", "Nätmask:", mask)).item(inp("gw", "Standardgateway:", gw)).item(inp("dns", "Önskad DNS-server:", dns));
        d.btn(Btn::primary("OK", Act::ApplyIpv4(id.to_string()))).btn(Btn::new("Avbryt", Act::Close))
    }

    pub fn apply_ipv4(&mut self, id: &str, d: &mut Dialog) -> bool {
        let dhcp = d.radio("m") != Some(1);
        let ip = d.input("ip").trim().to_string();
        let mask = d.input("mask").trim().to_string();
        let gw = d.input("gw").trim().to_string();
        let dns = d.input("dns").trim().to_string();
        if !dhcp && (!U::is_ip(&ip) || !U::is_ip(&mask) || !U::is_valid_mask(&mask) || (!gw.is_empty() && !U::is_ip(&gw)) || (!dns.is_empty() && !U::is_ip(&dns))) {
            d.error = "Kontrollera fälten – adresserna skrivs som a.b.c.d och nätmasken måste vara giltig.".into();
            return true;
        }
        let label;
        {
            let run = self.run.as_mut().unwrap();
            let h = match run.st.devices.get_mut(id) {
                Some(h) => h,
                None => return false,
            };
            label = if h.label.is_empty() { id.to_string() } else { h.label.clone() };
            if let Some(nic) = &mut h.nic {
                if dhcp {
                    nic.dhcp = true;
                } else {
                    let old_dns = nic.static_.as_ref().map(|s| s.dns.clone()).unwrap_or_default();
                    nic.dhcp = false;
                    nic.static_ = Some(StaticIp { ip, mask, gw: if gw.is_empty() { None } else { Some(gw) }, dns: if dns.is_empty() { old_dns } else { vec![dns] } });
                }
            }
            if !dhcp {
                h.lease = None;
            }
            S::touch(&mut run.st);
            if dhcp {
                S::dhcp(&mut run.st, id);
            }
            S::refresh(&mut run.st);
            run.commands += 1;
        }
        self.toast(format!("Inställningarna för {} har sparats.", label), ToastKind::Info);
        self.check_tasks();
        false
    }

    pub fn cable_dialog(&self, link_id: &str, state: crate::net::model::LinkState, label: &str) -> Dialog {
        use crate::net::model::LinkState;
        let mut d = Dialog::new("Nätverkskabeln").text(label.to_string());
        if state == LinkState::Unplugged {
            d = d.tone("Kabeln är urdragen.", Tone::Muted);
            d = d.btn(Btn::primary("Sätt tillbaka kabeln", Act::SetCable(link_id.to_string(), true))).btn(Btn::new("Byt patchkabel", Act::ReplaceCable(link_id.to_string())));
        } else {
            d = d.tone("Kabeln ser hel ut, men det syns inte alltid utanpå. Lyser lampan vid uttaget? Grönt = länk.", Tone::Muted);
            d = d.btn(Btn::primary("Byt patchkabel", Act::ReplaceCable(link_id.to_string()))).btn(Btn::new("Dra ur kabeln", Act::SetCable(link_id.to_string(), false)));
        }
        d.btn(Btn::new("Låt den vara", Act::Close))
    }

    pub fn rack_dialog(&self, name: &str) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let (title, ids): (&str, &[&str]) = match name {
            "A" => ("Rack A – nät", &["SW1", "SW2", "R1", "WLC", "LB", "RM"]),
            "B" => ("Rack B – servrar", &[]),
            _ => ("Lagrets rack", &["RB", "SWB"]),
        };
        let mut d = Dialog::new(title).tone("Frontpanelerna på nära håll. Välj en nätverksenhet för att sätta i konsolkabeln.", Tone::Muted);
        if ids.is_empty() {
            d = d.text("Servrarna: filserver, NTP, logg och ekonomisystem. De har inga konsolportar du behöver här.");
        }
        let mut rows: Vec<Vec<String>> = vec![vec!["Enhet".into(), "Typ".into(), "Portar uppe".into()]];
        for id in ids {
            if let Some(dev) = run.st.devices.get(*id) {
                let hn = dev.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| match dev.kind {
                    Kind::Lb => "LB-Nordvik".into(),
                    Kind::Wlc => "WLC-Nordvik".into(),
                    _ => if dev.label.is_empty() { id.to_string() } else { dev.label.clone() },
                });
                let kind = match dev.kind {
                    Kind::Router => "router",
                    Kind::Switch => "switch",
                    Kind::Wlc => "trådlös controller",
                    Kind::Lb => "lastbalanserare",
                    _ => "enhet",
                };
                rows.push(vec![hn, kind.into(), dev.model.clone()]);
            }
        }
        if rows.len() > 1 {
            d = d.item(Item::Table(rows));
        }
        for id in ids {
            if let Some(dev) = run.st.devices.get(*id) {
                let hn = dev.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| match dev.kind {
                    Kind::Lb => "LB-Nordvik".into(),
                    Kind::Wlc => "WLC-Nordvik".into(),
                    _ => id.to_string(),
                });
                d = d.btn(Btn::new(&format!("Konsol: {}", hn), Act::ConnectConsole(id.to_string())));
            }
        }
        if name == "A" {
            if let Some(l) = run.st.links.iter().find(|l| l.a.dev == "SW1" && l.a.port == "GigabitEthernet0/25") {
                let plugged = l.state != crate::net::model::LinkState::Unplugged;
                d = d.btn(Btn::new(if plugged { "Dra ur reservkabeln Gi0/25" } else { "Sätt tillbaka reservkabeln Gi0/25" }, Act::SetCable(l.id.clone(), !plugged)));
            }
        }
        d.btn(Btn::primary("Stäng", Act::Close))
    }

    // ------------------------------------------------------------------ mobilen och övervakningen

    pub fn phone_dialog(&mut self) -> Dialog {
        let mut d = Dialog::new("Gästmobilen");
        let run = match &mut self.run {
            Some(r) => r,
            None => return d.btn(Btn::primary("Stäng", Act::Close)),
        };
        if !run.st.devices.contains_key("Gast-mobil") {
            return d.text("Ingen mobil här.").btn(Btn::primary("Stäng", Act::Close));
        }
        let derived = S::get(&mut run.st);
        let any_ap = derived.ap_joined.values().any(|v| *v);
        let ssids: Vec<String> = if any_ap { run.st.devices.get("WLC").and_then(|w| w.wlc.as_ref()).map(|w| w.wlans.values().filter(|x| x.enabled).map(|x| x.ssid.clone()).collect()).unwrap_or_default() } else { vec![] };
        let ph = &run.st.devices["Gast-mobil"];
        let wifi = derived.wifi.get("Gast-mobil").cloned();
        let c = S::host_ip_conf(ph);
        d = d.tone("Wi-Fi", Tone::Head);
        if ssids.is_empty() {
            d = d.tone("Inga nätverk hittades.", Tone::Muted);
        }
        for s in &ssids {
            let on = ph.ssid.as_deref() == Some(s.as_str()) && wifi.is_some();
            d = d.btn(Btn { label: format!("{}{} – {}", if on { "✓ " } else { "" }, s, if on { "anslut igen" } else { "anslut" }), primary: false, act: Act::PhoneJoin(s.clone()), keep: false });
        }
        if let (Some(ssid), Some(w)) = (&ph.ssid, &wifi) {
            d = d.text(format!("Nätverk: {}\nIP-adress: {}\nRouter: {}\nSignal: {}", ssid, c.as_ref().and_then(|c| c.ip.clone()).unwrap_or("–".into()), c.as_ref().and_then(|c| c.gw.clone()).unwrap_or("–".into()), if w.interference { "svag, störningar" } else { "utmärkt" }));
            d = d.btn(Btn::new("Öppna www.example.com", Act::PhoneWeb)).btn(Btn::new("Sök efter enheter i nätet", Act::PhoneScan));
        }
        d.btn(Btn::primary("Lägg tillbaka", Act::Close))
    }

    pub fn phone_join(&mut self, ssid: &str) {
        if let Some(run) = &mut self.run {
            if let Some(ph) = run.st.devices.get_mut("Gast-mobil") {
                ph.ssid = Some(ssid.to_string());
            }
            S::touch(&mut run.st);
            S::dhcp(&mut run.st, "Gast-mobil");
            S::refresh(&mut run.st);
        }
        let d = self.phone_dialog();
        self.push_dlg(d);
        self.check_tasks();
    }

    pub fn phone_web(&mut self) {
        let ok = {
            let run = self.run.as_mut().unwrap();
            let r = S::resolve(&mut run.st, "Gast-mobil", "www.example.com");
            match (&r.error, &r.ip) {
                (None, Some(ip)) => {
                    let ip = ip.clone();
                    S::ping(&mut run.st, "Gast-mobil", &ip, PingOpts::tcp(443)).ok
                }
                _ => false,
            }
        };
        self.toast(if ok { "Sidan laddades: \"Example Domain\"" } else { "Det gick inte att öppna sidan." }, if ok { ToastKind::Good } else { ToastKind::Warn });
    }

    pub fn phone_scan(&mut self) {
        let found: Vec<String> = {
            let run = self.run.as_mut().unwrap();
            let mut f = vec![];
            for (ip, name) in [("192.168.2.20", "Lagerdatorn"), ("192.168.1.10", "Filserver"), ("192.168.2.1", "Router (lager)")] {
                if S::ping(&mut run.st, "Gast-mobil", ip, PingOpts::default()).ok {
                    f.push(format!("{} ({})", name, ip));
                }
            }
            f
        };
        self.toast(if found.is_empty() { "Inga andra enheter syns.".to_string() } else { format!("Hittade: {}", found.join(", ")) }, ToastKind::Info);
    }

    pub fn monitor_dialog(&mut self) -> Dialog {
        let mut d = Dialog::new("Övervakningen").wide();
        d = d.tone("Titta på grafen först, inte på enheten. En kurva som går rakt upp i taket är en loop. Sågtänder betyder en port som går upp och ner.", Tone::Muted);
        let run = match &mut self.run {
            Some(r) => r,
            None => return d.btn(Btn::primary("Stäng", Act::Close)),
        };
        let derived = S::get(&mut run.st);
        let storm: Vec<String> = derived.storm.iter().map(|(v, sws)| format!("VLAN {}: {}", v, sws.join(", "))).collect();
        d = d.tone("Trafik", Tone::Head);
        if storm.is_empty() {
            d = d.tone("Trafiken ser normal ut. Inga broadcaststormar.", Tone::Good);
        } else {
            for s in storm {
                d = d.tone(format!("BROADCASTSTORM i {}", s), Tone::Bad);
            }
        }
        let has_crypto = run.st.devices.get("R1").and_then(|x| x.config.as_ref()).map(|c| c.crypto.is_some()).unwrap_or(false);
        if has_crypto {
            let cs = S::crypto_status(&mut run.st, "R1");
            d = d.tone("VPN Göteborg <-> Borås", Tone::Head);
            let mut rows = vec![vec!["Fas 1".to_string(), "Tillstånd".to_string()]];
            for i in &cs.isakmp {
                rows.push(vec![format!("{} -> {}", i.src, i.dst), i.state.clone()]);
            }
            if cs.isakmp.is_empty() {
                rows.push(vec!["(ingen)".into(), "-".into()]);
            }
            d = d.item(Item::Table(rows));
            let mut rows = vec![vec!["Fas 2 (nät)".to_string(), "Status".to_string(), "encaps".to_string(), "decaps".to_string()]];
            for a in &cs.sas {
                rows.push(vec![format!("{} <-> {}", a.local, a.peer), if a.up { "uppe".into() } else { "nere".into() }, format!("{}", a.encaps as i64), format!("{}", a.decaps as i64)]);
            }
            d = d.item(Item::Table(rows));
        }
        if run.st.devices.get("LB").map(|x| x.lb.is_some()).unwrap_or(false) {
            d = d.tone("Lastbalanserare LB-Nordvik", Tone::Head);
            let mut rows = vec![vec!["Pool".to_string(), "Server".to_string(), "Status".to_string(), "Anrop".to_string()]];
            for p in S::lb_status(&mut run.st, "LB") {
                for m in p.members {
                    rows.push(vec![p.name.clone(), m.m.name.clone(), if !m.m.enabled { "avstängd".into() } else if m.up { "UP".into() } else { "DOWN".into() }, m.req.to_string()]);
                }
            }
            d = d.item(Item::Table(rows));
        }
        d.btn(Btn::primary("Stäng", Act::Close))
    }

    pub fn whiteboard_dialog(&self) -> Dialog {
        let w = self.run.as_ref().map(|r| r.n).unwrap_or(1);
        let best = self.save.quiz.get(&w).copied();
        let mut d = Dialog::new("Tavlan").text("På tavlan står veckans tema och en skiss över nätet. Vill du testa dig själv med några frågor?");
        if let Some(b) = best {
            d = d.tone(format!("Ditt bästa resultat för vecka {}: {} rätt", w, b), Tone::Muted);
        }
        d.btn(Btn::primary("Frågesport", Act::QuizStart(w))).btn(Btn::new("Läs tavlan", Act::Briefing)).btn(Btn::new("Stäng", Act::Close))
    }

    // ------------------------------------------------------------------ ärendelista, ledtrådar och felrapporter

    pub fn tasks_dialog(&self) -> Dialog {
        let run = match &self.run {
            Some(r) => r,
            None => return Dialog::new("Ärenden").text("Starta en vecka först.").btn(Btn::primary("Stäng", Act::Close)),
        };
        let w = match &run.week {
            Some(w) => w,
            None => return Dialog::new("Ärenden").text("Fri träning – inga ärenden.").btn(Btn::primary("Stäng", Act::Close)),
        };
        let mut d = Dialog::new(&format!("{} – {}", if w.build.is_some() { "Byggsteg" } else { "Ärenden" }, levels_name(w).to_lowercase())).wide();
        let mut rows = vec![vec!["".to_string(), "Ärende".to_string(), "Från".to_string(), "Status".to_string()]];
        for (i, t) in w.tasks.iter().enumerate() {
            let s = &run.ts[i];
            rows.push(vec![
                if t.build { "bygg".into() } else if t.krabba { "krabba".into() } else { "ärende".into() },
                if s.known { t.title.to_string() } else { "???".into() },
                t.npc.to_string(),
                if s.reported { format!("Klar ({}/2)", s.score) } else if s.fixed { "Löst – skriv rapport (F)".to_string() } else { "Öppen".to_string() },
            ]);
        }
        d = d.item(Item::Table(rows));
        if self.hard() {
            d = d.tone("SVÅRT LÄGE – bevis krävs. Ett fel räknas som löst först när det är rättat och du sedan kör beviskommandot:", Tone::Bad);
            for (i, t) in w.tasks.iter().enumerate() {
                if !run.ts[i].fixed && (run.ts[i].known || t.build) {
                    d = d.text(format!("• {}: {}", clip_str(t.title, 40), self.proof_text(t.id)));
                }
            }
        } else {
            d = d.tone("Utropstecken över en person betyder att hen har ett ärende. Krabba-felen hittar du genom att lyssna på kollegorna och felsöka.", Tone::Muted);
        }
        if let Some(ch) = w.challenge {
            if !run.exam {
                let done = self.challenge_marked(run.n);
                d = d.tone(format!("Veckans utmaning: {} – {}", ch, if done { "klarad (75 XP)" } else if super::levels::challenge_need(run.n) { "ej klar än" } else { "avgörs när veckan är klar" }), if done { Tone::Good } else { Tone::Muted });
            }
        }
        for (i, t) in w.tasks.iter().enumerate() {
            if run.ts[i].reported && !t.build {
                d = d.btn(Btn::new(&format!("Rapport: {}", clip_str(t.title, 22)), Act::ViewReport(i)));
            }
        }
        let mut copy = Btn::new("Kopiera felrapport.md", Act::CopyReport);
        copy.keep = true;
        d.btn(copy).btn(Btn::primary("Stäng", Act::Close))
    }

    pub fn report_view_dialog(&self, i: usize) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let t = &run.week.as_ref().unwrap().tasks[i];
        let r = run.ts[i].report.clone().unwrap_or_default();
        Dialog::new(&format!("Felrapport – {}", t.title))
            .wide()
            .item(Item::Text("Vad vi såg".into(), Tone::Head))
            .text(r.saw)
            .item(Item::Text("Vad vi trodde först".into(), Tone::Head))
            .text(r.first)
            .item(Item::Text("Vad vi kontrollerade".into(), Tone::Head))
            .tone(r.checked, Tone::Code)
            .item(Item::Text("Vad felet var".into(), Tone::Head))
            .text(r.cause)
            .item(Item::Text("Hur det skulle rättas".into(), Tone::Head))
            .text(r.fix)
            .btn(Btn::primary("Tillbaka", Act::Tasks))
    }

    pub fn open_hint_picker(&mut self) {
        let run = match &self.run {
            Some(r) => r,
            None => return,
        };
        let w = match &run.week {
            Some(w) => w,
            None => {
                self.toast("Inga ärenden i fri träning.", ToastKind::Info);
                return;
            }
        };
        if run.exam {
            self.toast("Inga ledtrådar i examensläget. Handboken (H) får du använda.", ToastKind::Warn);
            return;
        }
        let open: Vec<usize> = (0..w.tasks.len()).filter(|i| !run.ts[*i].fixed && run.ts[*i].known).collect();
        if open.is_empty() {
            self.toast("Prata med kollegorna (utropstecken) för att få veta vad som är fel.", ToastKind::Info);
            return;
        }
        if open.len() == 1 {
            let d = self.hint_dialog(open[0]);
            self.push_dlg(d);
            return;
        }
        let mut d = Dialog::new("Ledtråd").text("Vilket ärende vill du ha en ledtråd till? Varje ledtråd kan kosta en stjärna.");
        for i in open {
            d = d.btn(Btn::new(&clip_str(w.tasks[i].title, 40), Act::OpenHint(i)));
        }
        d = d.btn(Btn::primary("Stäng", Act::Close));
        self.push_dlg(d);
    }

    pub fn hint_dialog(&self, i: usize) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let t = &run.week.as_ref().unwrap().tasks[i];
        let s = &run.ts[i];
        let mut d = Dialog::new(&format!("Ledtråd – {}", t.title)).wide();
        d = d.tone(t.ticket, Tone::Muted).tone("FASTNAR DU HÄR?", Tone::Head);
        for (k, h) in t.hints.iter().enumerate() {
            if (k as u32) < s.hints {
                d = d.text(format!("{}. {}", k + 1, h));
            } else {
                d = d.tone(format!("{}. (dold ledtråd)", k + 1), Tone::Muted);
            }
        }
        d = d.tone("Fler än två ledtrådar under veckan kostar en stjärna.", Tone::Muted);
        if (s.hints as usize) < t.hints.len() {
            d = d.btn(Btn::primary("Visa nästa ledtråd", Act::NextHint(i)));
        }
        d.btn(Btn::new("Stäng", Act::Close))
    }

    pub fn open_report_picker(&mut self) {
        let run = match &self.run {
            Some(r) => r,
            None => return,
        };
        let w = match &run.week {
            Some(w) => w,
            None => {
                self.toast("Felrapporter skrivs bara under veckorna.", ToastKind::Info);
                return;
            }
        };
        let ready: Vec<usize> = (0..w.tasks.len()).filter(|i| run.ts[*i].fixed && !run.ts[*i].reported && !w.tasks[*i].build).collect();
        if ready.is_empty() {
            if self.hard() {
                self.toast("Inget fel är bevisat än. Rätta felet och kör beviskommandot (se ärendelistan, Tab) först.", ToastKind::Info);
            } else {
                self.toast("Inget ärende är löst ännu. Lös ett fel först, sedan skriver du rapporten här.", ToastKind::Info);
            }
            return;
        }
        if ready.len() == 1 {
            let d = self.report_dialog(ready[0]);
            self.push_dlg(d);
            return;
        }
        let mut d = Dialog::new("Felrapport").text("Vilket fel vill du rapportera?");
        for i in ready {
            d = d.btn(Btn::new(&clip_str(w.tasks[i].title, 40), Act::ReportPick(i)));
        }
        d = d.btn(Btn::primary("Senare", Act::Close));
        self.push_dlg(d);
    }

    pub fn report_dialog(&self, i: usize) -> Dialog {
        let t = self.run.as_ref().unwrap().week.as_ref().unwrap().tasks[i].clone();
        let checked: Vec<String> = self.commands_for();
        let last: Vec<String> = checked.iter().rev().take(8).rev().cloned().collect();
        let checked = last.join("\n");
        let rep = t.report.clone().unwrap_or(super::levels::Report { cause: vec!["(saknas)"], fix: vec!["(saknas)"] });
        let saw = strip_speaker(t.ticket).replace('"', "");
        let mk = |id: &'static str, label: &str, v: String, rows: usize, hint: &str| {
            let n = v.chars().count();
            Item::Input { id, label: label.into(), value: v, rows, hint: hint.into(), cur: n }
        };
        let order_c = shuffled(&rep.cause, t.id);
        let order_f = shuffled(&rep.fix, &format!("{}f", t.id));
        Dialog::new(&format!("Felrapport – {}", t.title))
            .wide()
            .tone("Felrapport enligt Krabba-passets fem fält: vad du såg, vad du trodde först, vad du kontrollerade, vad felet var och hur det skulle rättas. Tab flyttar mellan fälten.", Tone::Muted)
            .item(mk("r1", "1. Vad såg du?", saw, 2, "Symptomet, t.ex. vad användaren sa och vad en show-utskrift visade"))
            .item(mk("r2", "2. Vad trodde du först?", String::new(), 2, "Din första hypotes"))
            .item(mk("r3", "3. Vad kontrollerade du?", checked, 4, ""))
            .item(Item::Radio { id: "cause", legend: "4. Vad var felet?".into(), options: order_c.iter().map(|k| rep.cause[*k].to_string()).collect(), sel: None })
            .item(Item::Radio { id: "fix", legend: "5. Hur skulle det rättas?".into(), options: order_f.iter().map(|k| rep.fix[*k].to_string()).collect(), sel: None })
            .btn(Btn::primary("Lämna in", Act::ReportSubmit(i)))
            .btn(Btn::new("Senare", Act::Close))
            .tag(i)
    }

    pub fn submit_report_dialog(&mut self, i: usize, d: &mut Dialog) -> bool {
        let t = self.run.as_ref().unwrap().week.as_ref().unwrap().tasks[i].clone();
        let rep = t.report.clone().unwrap();
        let (c, f) = match (d.radio("cause"), d.radio("fix")) {
            (Some(c), Some(f)) => (c, f),
            _ => {
                d.error = "Välj ett svar på fråga 4 och 5.".into();
                return true;
            }
        };
        let order_c = shuffled(&rep.cause, t.id);
        let order_f = shuffled(&rep.fix, &format!("{}f", t.id));
        let (ci, fi) = (order_c[c], order_f[f]);
        let r = ReportData { saw: d.input("r1"), first: d.input("r2"), checked: d.input("r3"), cause: rep.cause[ci].to_string(), fix: rep.fix[fi].to_string(), score: (if ci == 0 { 1 } else { 0 }) + (if fi == 0 { 1 } else { 0 }) };
        let fb = self.report_feedback_dialog(i, &r);
        self.submit_report(i, r);
        self.push_dlg(fb);
        false
    }

    pub fn report_feedback_dialog(&self, i: usize, r: &ReportData) -> Dialog {
        let t = &self.run.as_ref().unwrap().week.as_ref().unwrap().tasks[i];
        let rep = t.report.clone().unwrap();
        let ok_c = r.cause == rep.cause[0];
        let ok_f = r.fix == rep.fix[0];
        let mut d = Dialog::new(if ok_c && ok_f { "Helt rätt!" } else { "Rapporten är inlämnad" }).wide();
        d = d.tone(format!("{} Vad felet var: {}", if ok_c { "[rätt]" } else { "[fel]" }, rep.cause[0]), if ok_c { Tone::Good } else { Tone::Bad });
        if !ok_c {
            d = d.tone(format!("Du svarade: {}", r.cause), Tone::Muted);
        }
        d = d.tone(format!("{} Hur det skulle rättas: {}", if ok_f { "[rätt]" } else { "[fel]" }, rep.fix[0]), if ok_f { Tone::Good } else { Tone::Bad });
        if !ok_f {
            d = d.tone(format!("Du svarade: {}", r.fix), Tone::Muted);
        }
        d = d.tone("SÅ HÄR LÖSER DU DET", Tone::Head);
        if let Some(h) = t.hints.get(1) {
            d = d.text(h.to_string());
        }
        if let Some(h) = t.hints.get(2) {
            d = d.text(h.to_string());
        }
        d.btn(Btn::primary("Fortsätt", Act::Close))
    }

    pub fn report_nag_dialog(&self, all: bool) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let w = run.week.as_ref().unwrap();
        let pend = self.pending_reports();
        let mut d = Dialog::new("Dags för felrapporter");
        if all {
            d = d.tone("Alla fel är lösta – bra jobbat!", Tone::Good).text("Veckan är klar först när du har skickat in en felrapport för varje fel. Det här saknas:");
        } else {
            d = d.tone("Alla Krabba-fel är lösta!", Tone::Good).text("Skicka in felrapporterna för dem nu, medan du minns vad du gjorde. Sedan är det bara kollegans ärende kvar. Rapporter som saknas:");
        }
        for i in pend {
            d = d.text(format!("• {}", w.tasks[i].title));
        }
        d.tone("Tryck F när som helst (inte när terminalen är öppen), eller klicka på den gula rutan uppe till vänster.", Tone::Muted).btn(Btn::primary("Skriv felrapport nu", Act::ReportNow)).btn(Btn::new("Senare", Act::Close))
    }

    pub fn week_done_dialog(&self) -> Dialog {
        let run = self.run.as_ref().unwrap();
        let res = self.result();
        let w = run.week.as_ref();
        let build = w.map(|w| w.build.is_some()).unwrap_or(false);
        let mut d = Dialog::new(&format!("{} klar!", super::levels::name(w))).tone(format!("{} / 3 stjärnor", res.stars), Tone::Big);
        d = d.text(format!("Du löste alla {} på {} min med {} kommandon och {} ledtrådar.", if build { "byggsteg" } else { "ärenden" }, res.minutes, res.commands, res.hints));
        if !build {
            d = d.text(format!("Felrapporterna: {} av {} rätt.", res.report_score, res.report_max));
        }
        if !res.badges.is_empty() {
            d = d.text(format!("Utmärkelser: {}", res.badges.join(", ")));
        }
        if let Some((t, ok)) = &res.challenge {
            d = d.tone(format!("Veckans utmaning – {}: {}", t, if *ok { "klarad! +75 XP" } else { "inte den här gången" }), if *ok { Tone::Good } else { Tone::Muted });
        }
        d = d.tone(if build { "Nätet fungerar – och du byggde det själv." } else { "Kopiera felrapport.md och lägg den i veckans mapp i ditt repo, precis som efter ett riktigt Krabba-pass." }, Tone::Muted);
        let mut copy = Btn::new("Kopiera felrapport.md", Act::CopyReport);
        copy.keep = true;
        d.btn(copy).btn(Btn::primary("Till menyn", Act::Menu)).btn(Btn::new("Stanna kvar", Act::Close))
    }

    // ------------------------------------------------------------------ inställningar, anteckningar, hjälp, karta

    pub fn settings_dialog(&self) -> Dialog {
        let s = &self.save.settings;
        let diff = ["easy", "normal", "hard"].iter().position(|x| *x == s.difficulty).unwrap_or(1);
        let theme = ["classic", "green", "amber", "pixel"].iter().position(|x| *x == s.term_theme).unwrap_or(0);
        let sizes = ["16", "18", "20", "22", "24", "26", "28"];
        let sz = sizes.iter().position(|x| x.parse::<u32>().unwrap_or(22) >= s.term_font).unwrap_or(3);
        Dialog::new("Inställningar")
            .item(Item::Choice { id: "diff", label: "Svårighetsgrad".into(), options: vec!["Lätt".into(), "Normal".into(), "Svår".into()], sel: diff })
            .item(Item::Choice { id: "theme", label: "Terminalens utseende".into(), options: vec!["Klassiskt".into(), "Grön fosfor".into(), "Bärnsten".into(), "Pixel (16-bit)".into()], sel: theme })
            .item(Item::Choice { id: "font", label: "Terminalens textstorlek".into(), options: sizes.iter().map(|x| x.to_string()).collect(), sel: sz })
            .item(Item::Check { id: "fast", label: "Snabb utskrift i terminalen (korta väntetider för ping m.m.)".into(), on: s.fast_output })
            .item(Item::Check { id: "keys", label: "Visa tangentraden längst ned".into(), on: s.show_keys })
            .item(Item::Check { id: "hud", label: "Visa HUD (U växlar)".into(), on: s.hud })
            .item(Item::Choice { id: "weather", label: "Väder".into(), options: vec!["Varierar".into(), "Alltid sol".into()], sel: if s.weather == "sol" { 1 } else { 0 } })
            .item(Item::Check { id: "mini", label: "Visa minikartan (K växlar)".into(), on: s.minimap })
            .item(Item::Check { id: "sound", label: "Ljud på".into(), on: s.sound })
            .item(Item::Choice { id: "vol", label: "Volym".into(), options: LEVELS.iter().map(|x| format!("{} %", (x * 100.0).round())).collect(), sel: level_idx(s.volume) })
            .item(Item::Check { id: "typesnd", label: "Skrivljud i terminalen".into(), on: s.type_sound })
            .item(Item::Check { id: "music", label: "Musik".into(), on: s.music })
            .item(Item::Choice { id: "mvol", label: "Musikvolym".into(), options: LEVELS.iter().map(|x| format!("{} %", (x * 100.0).round())).collect(), sel: level_idx(s.music_vol) })
            .item(Item::Choice { id: "fx", label: "Effekter".into(), options: LEVELS.iter().map(|x| format!("{} %", (x * 100.0).round())).collect(), sel: level_idx(s.fx) })
            .item(Item::Choice { id: "amb", label: "Miljöljud".into(), options: LEVELS.iter().map(|x| format!("{} %", (x * 100.0).round())).collect(), sel: level_idx(s.ambience) })
            .tone("Lätt: ledtrådar kostar inga poäng. Svår: ingen pil, markör eller utropstecken, och fel måste bevisas med ett kommando. XP ×1,5.", Tone::Muted)
            .btn(Btn::primary("Klar", Act::SettingsApply))
            .btn(Btn::new("Avbryt", Act::Close))
    }

    pub fn apply_settings(&mut self, d: &Dialog) {
        let diff = ["easy", "normal", "hard"][d.choice("diff").min(2)];
        let theme = ["classic", "green", "amber", "pixel"][d.choice("theme").min(3)];
        let sizes = [16u32, 18, 20, 22, 24, 26, 28];
        self.save.settings.difficulty = diff.into();
        self.save.settings.term_theme = theme.into();
        self.save.settings.term_font = sizes[d.choice("font").min(6)];
        self.save.settings.fast_output = d.checked("fast");
        self.save.settings.show_keys = d.checked("keys");
        self.save.settings.hud = d.checked("hud");
        self.save.settings.minimap = d.checked("mini");
        self.save.settings.weather = if d.choice("weather") == 1 { "sol".into() } else { "auto".into() };
        let s = &mut self.save.settings;
        s.sound = d.checked("sound");
        s.volume = LEVELS[d.choice("vol").min(LEVELS.len() - 1)];
        s.type_sound = d.checked("typesnd");
        s.music = d.checked("music");
        s.music_vol = LEVELS[d.choice("mvol").min(LEVELS.len() - 1)];
        s.fx = LEVELS[d.choice("fx").min(LEVELS.len() - 1)];
        s.ambience = LEVELS[d.choice("amb").min(LEVELS.len() - 1)];
        crate::engine::audio::set_settings(s.audio());
        self.apply_weather();
        self.term.font = self.save.settings.term_font as f32;
        self.term.theme = self.save.settings.term_theme.clone();
        self.refresh_markers();
        self.save.save();
    }

    pub fn notes_dialog(&self) -> Dialog {
        let n = self.save.notes.chars().count();
        Dialog::new("Anteckningar")
            .wide()
            .tone("Dina egna anteckningar – som dagbok.md. De sparas när du stänger rutan.", Tone::Muted)
            .item(Item::Input { id: "notes", label: String::new(), value: self.save.notes.clone(), rows: 12, hint: "Vad fastnade du på? Vilka kommandon vill du komma ihåg?".into(), cur: n })
            .btn(Btn::primary("Spara och stäng", Act::NotesSave))
            .tag(1)
    }

    pub fn help_dialog(&self) -> Dialog {
        let mut d = Dialog::new("Hjälp").wide();
        d.items = blocks_to_items(data::KEYS);
        d.btn(Btn::primary("Stäng", Act::Close))
    }

    pub fn map_dialog(&self) -> Dialog {
        let mut d = Dialog::new("Karta och snabbresa");
        let run = match &self.run {
            Some(r) => r,
            None => return d.btn(Btn::primary("Stäng", Act::Close)),
        };
        let here = match run.world.site.as_str() {
            "boras" => "lagret i Borås",
            "molndal" => "Mölndalskontoret",
            _ => "huvudkontoret i Göteborg",
        };
        d = d.text(format!("Du är på {}.", here));
        d = d.tone("Åk till:", Tone::Muted);
        if run.world.site != "gbg" {
            d = d.btn(Btn::new("Göteborg (huvudkontoret)", Act::Travel("gbg".into())));
        }
        if run.world.site != "boras" {
            d = d.btn(Btn::new("Borås (lagret)", Act::Travel("boras".into())));
        }
        if run.st.devices.contains_key("RM") && run.world.site != "molndal" {
            d = d.btn(Btn::new("Mölndal", Act::Travel("molndal".into())));
        }
        d.btn(Btn::primary("Stäng", Act::Close))
    }

    // ------------------------------------------------------------------ frågesport

    pub fn start_quiz(&mut self, week: u32) {
        let mut items: Vec<QItem> = vec![];
        if let Some(w) = self.run.as_ref().and_then(|r| r.week.as_ref()) {
            for q in &w.quiz {
                items.push(QItem { q: q.q.to_string(), opts: q.options.iter().map(|s| s.to_string()).collect(), answer: q.answer, explain: q.explain.to_string() });
            }
        }
        if items.is_empty() {
            let list = data::QUIZ.iter().find(|(w, _)| *w == week).or_else(|| data::QUIZ.first());
            if let Some((_, qs)) = list {
                for (q, right, w1, w2) in qs.iter() {
                    let mut opts = vec![right.to_string(), w1.to_string(), w2.to_string()];
                    opts.sort_by_key(|o| U::hash(&format!("{}{}", q, o)));
                    let answer = opts.iter().position(|o| o == right).unwrap_or(0);
                    items.push(QItem { q: q.to_string(), opts, answer, explain: String::new() });
                }
            }
        }
        if items.is_empty() {
            self.toast("Inga frågor för den här veckan.", ToastKind::Info);
            return;
        }
        self.quiz = Some(QuizRun { week, items, i: 0, right: 0 });
        let d = self.quiz_question_dialog();
        self.push_dlg(d);
    }

    fn quiz_question_dialog(&self) -> Dialog {
        let q = self.quiz.as_ref().unwrap();
        let it = &q.items[q.i];
        Dialog::new(&format!("Frågesport – fråga {} av {}", q.i + 1, q.items.len()))
            .wide()
            .tone(it.q.clone(), Tone::Quote)
            .item(Item::Radio { id: "a", legend: "Välj ett svar".into(), options: it.opts.clone(), sel: None })
            .btn(Btn::primary("Svara", Act::QuizAnswer))
            .btn(Btn::new("Avbryt", Act::Close))
    }

    pub fn quiz_answer(&mut self, d: &mut Dialog) -> bool {
        let sel = match d.radio("a") {
            Some(s) => s,
            None => {
                d.error = "Välj ett svar först.".into();
                return true;
            }
        };
        let (ok, explain, correct_text) = {
            let q = self.quiz.as_mut().unwrap();
            let it = &q.items[q.i];
            let ok = sel == it.answer;
            if ok {
                q.right += 1;
            }
            (ok, it.explain.clone(), it.opts[it.answer].clone())
        };
        let last = {
            let q = self.quiz.as_ref().unwrap();
            q.i + 1 >= q.items.len()
        };
        crate::engine::audio::sfx(if ok { "success" } else { "fail" });
        let mut fb = Dialog::new(if ok { "Rätt!" } else { "Inte riktigt" });
        fb = if ok { fb.tone("Rätt svar!", Tone::Good) } else { fb.tone(format!("Rätt svar är: {}", correct_text), Tone::Bad) };
        if !explain.is_empty() {
            fb = fb.text(explain);
        }
        fb = fb.btn(Btn::primary(if last { "Se resultat" } else { "Nästa fråga" }, Act::QuizNext));
        self.push_dlg(fb);
        false
    }

    pub fn quiz_next(&mut self) {
        let finished = {
            let q = match &mut self.quiz {
                Some(q) => q,
                None => return,
            };
            q.i += 1;
            q.i >= q.items.len()
        };
        if !finished {
            let d = self.quiz_question_dialog();
            self.push_dlg(d);
            return;
        }
        let q = self.quiz.take().unwrap();
        let prev = self.save.quiz.get(&q.week).copied().unwrap_or(0);
        if q.right > prev {
            self.save.quiz.insert(q.week, q.right);
            self.save.save();
        }
        let n = q.items.len();
        let d = Dialog::new("Frågesport klar")
            .tone(format!("{} / {}", q.right, n), Tone::Big)
            .text(if q.right as usize == n { "Alla rätt!" } else { "Bra jobbat! Du kan göra om frågesporten när du vill." })
            .btn(Btn::primary("Stäng", Act::Close));
        self.push_dlg(d);
    }

    // ------------------------------------------------------------------ knapparnas handlingar

    /// Utför en knapps handling. Dialogen är redan borttagen från stacken; returnera true för att behålla den öppen.
    pub fn run_act(&mut self, act: Act, d: &mut Dialog) -> bool {
        match act {
            Act::Close => {}
            Act::Menu => self.leave_to_menu(),
            Act::StartWeek(n, exam) | Act::ConfirmStart(n, exam) => self.start_week(n, exam),
            Act::OpenHint(i) => {
                let dl = self.hint_dialog(i);
                self.push_dlg(dl);
            }
            Act::NextHint(i) => {
                if let Some(run) = &mut self.run {
                    run.ts[i].hints += 1;
                    run.last_progress = run.elapsed;
                }
                self.on_hint();
                let dl = self.hint_dialog(i);
                self.push_dlg(dl);
            }
            Act::HintPicker => self.open_hint_picker(),
            Act::ReportPick(i) => {
                let dl = self.report_dialog(i);
                self.push_dlg(dl);
            }
            Act::ReportSubmit(i) => return self.submit_report_dialog(i, d),
            Act::ReportNow => self.open_report_picker(),
            Act::ViewReport(i) => {
                let dl = self.report_view_dialog(i);
                self.push_dlg(dl);
            }
            Act::Tasks => {
                let dl = self.tasks_dialog();
                self.push_dlg(dl);
            }
            Act::CopyReport => {
                self.clip = Some(self.report_markdown());
                self.toast("Felrapporten är kopierad till urklipp.", ToastKind::Good);
                return true;
            }
            Act::OpenWin(id) => self.open_windows(&id),
            Act::OpenIpv4(id) => {
                let dl = self.ipv4_dialog(&id);
                self.push_dlg(dl);
            }
            Act::ApplyIpv4(id) => return self.apply_ipv4(&id, d),
            Act::ReplaceCable(id) => {
                let link = self.run.as_ref().and_then(|r| r.st.links.iter().find(|l| l.id == id).or_else(|| r.st.links.iter().find(|l| l.a.dev == id || l.b.dev == id)).map(|l| l.id.clone()));
                if let Some(l) = link {
                    self.set_cable(&l, true, true);
                }
            }
            Act::SetCable(id, plugged) => self.set_cable(&id, plugged, false),
            Act::ConnectConsole(id) => self.connect_console(&id),
            Act::Travel(site) => self.travel_pub(&site),
            Act::QuizStart(w) => self.start_quiz(w),
            Act::QuizAnswer => return self.quiz_answer(d),
            Act::QuizNext => self.quiz_next(),
            Act::SettingsOpen => {
                let dl = self.settings_dialog();
                self.push_dlg(dl);
            }
            Act::SettingsApply => self.apply_settings(d),
            Act::NotesSave => {
                self.save.notes = d.input("notes");
                self.save.save();
            }
            Act::Whiteboard => {
                let dl = self.whiteboard_dialog();
                self.push_dlg(dl);
            }
            Act::Briefing => {
                let dl = self.briefing_dialog(false);
                self.push_dlg(dl);
            }
            Act::Phone => {
                let dl = self.phone_dialog();
                self.push_dlg(dl);
            }
            Act::PhoneJoin(s) => self.phone_join(&s),
            Act::PhoneWeb => {
                self.phone_web();
                return true;
            }
            Act::PhoneScan => {
                self.phone_scan();
                return true;
            }
            Act::OpenLaptop => self.open_laptop(None),
            Act::Handbook => self.open_handbook("cmd"),
            Act::Help => {
                let dl = self.help_dialog();
                self.push_dlg(dl);
            }
            Act::MapOpen => {
                let dl = self.map_dialog();
                self.push_dlg(dl);
            }
            Act::ResetProgress => {
                self.save.progress.clear();
                self.save.save();
            }
            Act::Quit => self.quit = true,
            Act::Continue => {
                let _ = self.resume_run();
            }
        }
        false
    }

    pub fn travel_pub(&mut self, site: &str) {
        if let Some(run) = &mut self.run {
            run.world.set_site(site);
            run.site = site.to_string();
        }
        let msg = match site {
            "boras" => "Du är framme vid lagret i Borås.",
            "molndal" => "Du är framme på Mölndalskontoret.",
            _ => "Tillbaka på huvudkontoret i Göteborg.",
        };
        self.toast(msg, ToastKind::Info);
    }
}

fn levels_name(w: &super::levels::Week) -> String {
    super::levels::name(Some(w))
}

fn clip_str(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

#[allow(unused)]
fn _use(_: Overlay) {}
