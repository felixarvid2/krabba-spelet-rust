//! Veckorna (1–10), byggena (101–105) och kurs 2 (11–14). Port av js/levels.js, js/build.js och js/kurs2.js.
#![allow(unused_variables, dead_code)]
use crate::net::model::*;
use crate::net::sim::{self, AclPkt, PingOpts, State};
use crate::net::util as U;

pub type CheckFn = fn(&mut State) -> bool;
pub type SetupFn = fn(&mut State);

#[derive(Clone, Debug)]
pub struct Report {
    pub cause: Vec<&'static str>,
    pub fix: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: &'static str,
    /// Krabba-fel (true) eller kollegans ärende (false)
    pub krabba: bool,
    /// Byggsteg (bockas av automatiskt, ingen felrapport)
    pub build: bool,
    pub npc: &'static str,
    /// Enheten felet sitter i (för kompass/mål)
    pub target: &'static str,
    pub title: &'static str,
    pub ticket: &'static str,
    pub hints: Vec<&'static str>,
    /// Kontroll: är felet rättat? Spelets flaggor ligger i st.flags.misc (t.ex. "stpLooked").
    pub check: CheckFn,
    /// Svarsalternativ i felrapporten; första alternativet är rätt.
    pub report: Option<Report>,
    /// Flagga som spelet sätter när spelaren gjort något (t.ex. "stpLooked")
    pub flag: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Quiz {
    pub q: &'static str,
    pub options: Vec<&'static str>,
    /// Index för rätt svar
    pub answer: usize,
    pub explain: &'static str,
}

#[derive(Clone, Debug)]
pub struct Week {
    /// 1–14 för veckor, 101–105 för byggen
    pub week: u32,
    /// Byggets nummer (1–5) om det är ett bygge
    pub build: Option<u32>,
    /// 1 eller 2
    pub kurs: u32,
    pub title: &'static str,
    pub chapter: &'static str,
    /// Startplats: "gbg" eller "boras"
    pub site: &'static str,
    pub course: Option<&'static str>,
    pub intro: &'static str,
    pub learn: Vec<&'static str>,
    /// Förslag på kommandon i terminalen
    pub chips: Vec<&'static str>,
    pub tasks: Vec<Task>,
    pub quiz: Vec<Quiz>,
    /// Veckoutmaning (kurs 2), fritext
    pub challenge: Option<&'static str>,
}

pub const DHCP_HOSTS: &[&str] = &[
    "PC-Anna", "PC-Karim", "PC-Bo", "PC-Maja", "PC-Sara", "PC-Lisa", "Gast-laptop", "PC-Lager", "Gast-mobil", "Lager-skanner",
];

#[path = "levels_build.rs"]
mod build;
#[path = "levels_data.rs"]
mod data;
#[path = "levels_kurs2.rs"]
mod kurs2;
use build::*;
use kurs2::*;

// ------------------------------------------------------------------ Hjälpfunktioner

/// Kör en ändring på både running-config och startup-config
pub(crate) fn both(d: &mut Device, f: impl Fn(&mut IosConfig)) {
    if let Some(c) = d.config.as_mut() {
        f(c);
    }
    if let Some(c) = d.startup.as_mut() {
        f(c);
    }
}
pub(crate) fn iface<'a>(c: &'a mut IosConfig, n: &str) -> &'a mut Iface {
    c.ifaces.get_mut(n).unwrap_or_else(|| panic!("interface {} saknas", n))
}
pub(crate) fn ok(st: &mut State, from: &str, to: &str) -> bool {
    ok_with(st, from, to, PingOpts::default())
}
pub(crate) fn ok_with(st: &mut State, from: &str, to: &str, opts: PingOpts) -> bool {
    let r = sim::ping(st, from, to, opts);
    r.ok && r.loss < 0.9
}
fn lease_in(st: &State, host: &str, net: &str, mask: &str) -> bool {
    match st.devices.get(host).and_then(sim::host_ip_conf) {
        Some(c) => match &c.ip {
            Some(ip) => !c.apipa && U::in_net(ip, net, mask),
            None => false,
        },
        None => false,
    }
}
fn host_setup(st: &mut State, list: &[&str]) {
    for h in list {
        sim::dhcp(st, h);
    }
}
fn link_to_mut<'a>(st: &'a mut State, dev: &str) -> Option<&'a mut Link> {
    st.links.iter_mut().find(|l| l.b.dev == dev)
}


/// Alla veckor och byggen i menyordning (vecka 1–14, sedan byggena 101–105)
pub fn all() -> Vec<Week> {
    let mut v = data::weeks1();
    v.extend(data::weeks2());
    v.extend(data::builds());
    v
}
pub fn def(n: u32) -> Option<Week> { all().into_iter().find(|w| w.week == n) }
/// "Vecka 3", "Bygge 2" eller "Fri träning"
pub fn name(w: Option<&Week>) -> String {
    match w {
        None => "Fri träning".into(),
        Some(w) => match w.build { Some(b) => format!("Bygge {}", b), None => format!("Vecka {}", w.week) },
    }
}
pub fn is_build(n: u32) -> bool { n > 100 }
/// Det felfria nätet för en vecka, med DHCP-adresser utdelade (baseState)
pub fn base_state(week: u32) -> State {
    let mut st = State::new(build_golden(week));
    if let Some(d) = st.devices.get_mut("Gast-mobil") {
        d.ssid = Some("Nordvik-Gast".into());
    }
    if let Some(d) = st.devices.get_mut("Lager-skanner") {
        d.ssid = Some("Nordvik-Lager".into());
    }
    sim::refresh(&mut st);
    let mut hosts: Vec<&str> = DHCP_HOSTS.iter().copied().filter(|h| st.devices.contains_key(*h)).collect();
    if st.devices.contains_key("PC-Molndal") {
        hosts.push("PC-Molndal");
    }
    host_setup(&mut st, &hosts);
    sim::refresh(&mut st);
    st
}
/// Startar en vecka eller ett bygge: bygger nätet, sätter in felen och kör after()
pub fn start(n: u32) -> (State, Option<Week>) {
    let wk = def(n);
    let base = if is_build(n) { build_base(n) } else if n == 0 { 1 } else { n };
    let mut st = base_state(base);
    let Some(wk) = wk else { return (st, None) };
    // Ögonblicksbild först så att ändringarna ger loggrader
    sim::refresh(&mut st);
    setup(n, &mut st);
    sim::touch(&mut st);
    sim::refresh(&mut st);
    after(n, &mut st);
    sim::refresh(&mut st);
    (st, Some(wk))
}
fn setup(n: u32, st: &mut State) {
    match n {
        1 => su_w1(st),
        2 => su_w2(st),
        3 => su_w3(st),
        4 => su_w4(st),
        5 => su_w5(st),
        6 => su_w6(st),
        7 => su_w7(st),
        8 => su_w8(st),
        9 => su_w9(st),
        10 => su_w10(st),
        11 => su_w11(st),
        12 => su_w12(st),
        13 => su_w13(st),
        14 => su_w14(st),
        101 => wipe(st, "SW1"),
        102 => wipe(st, "R1"),
        103 => {
            wipe(st, "RB");
            wipe(st, "SWB");
        }
        104 => su_b4(st),
        105 => su_b5(st),
        _ => {}
    }
}
fn after(n: u32, st: &mut State) {
    match n {
        1 => host_setup(st, &["PC-Anna"]),
        2 => host_setup(st, &["PC-Karim", "PC-Bo"]),
        3 => host_setup(st, &["PC-Karim", "PC-Maja"]),
        8 => {
            sim::touch(st);
            host_setup(st, &["Gast-mobil"]);
        }
        10 => {
            // Drifttrafiken har redan byggt tunneln: fas 1 är uppe
            ping_ignore(st, "Tekniker", "192.168.2.193");
            ping_ignore(st, "AP-Lager-1", "192.168.1.196");
            // Nils har försökt nå filservern hela morgonen
            ping_ignore(st, "PC-Lager", "192.168.1.10");
        }
        11 => {
            ping_ignore(st, "PC-Molndal", "192.168.1.10");
            ping_ignore(st, "PC-Lager", "192.168.3.10");
        }
        12 => af_w12(st),
        13 => {
            ping_ignore(st, "PC-Lager", "192.168.1.10");
            ping_ignore(st, "PC-Molndal", "192.168.1.10");
        }
        14 => {
            ping_ignore(st, "PC-Lager", "9.9.9.9");
            sim::ping(st, "PC-Molndal", "198.51.100.80", PingOpts::tcp(80));
        }
        101..=105 => renew_all(st),
        _ => {}
    }
}
fn ping_ignore(st: &mut State, from: &str, to: &str) {
    sim::ping(st, from, to, PingOpts::default());
}

// ------------------------------------------------------------------ Vecka 1
fn su_w1(st: &mut State) {
    both(st.dev_mut("SW1"), |c| iface(c, "GigabitEthernet0/5").shutdown = true);
    both(st.dev_mut("SW1"), |c| c.lines.con.speed = 19200);
    let sw2 = st.dev_mut("SW2");
    if let Some(s) = sw2.startup.as_mut() {
        s.hostname = "Switch".into();
    }
    sw2.config = sw2.startup.clone();
    sw2.rt.boot_time = -540.0;
    sw2.rt.logs.clear();
    sw2.rt.reloads = 1;
}
pub(crate) fn ck_v1k1(st: &mut State) -> bool {
    let d = st.dev("SW1");
    d.cfg().lines.con.speed == 9600 && d.startup.as_ref().map(|s| s.lines.con.speed == 9600).unwrap_or(false)
}
pub(crate) fn ck_v1k2(st: &mut State) -> bool {
    let sd = !st.dev("SW1").cfg().ifaces["GigabitEthernet0/5"].shutdown;
    sd && ok(st, "PC-Anna", "192.168.1.10")
}
pub(crate) fn ck_v1n1(st: &mut State) -> bool {
    let d = st.dev("SW2");
    d.cfg().hostname == "SW-Nordvik-2" && d.startup.as_ref().map(|s| s.hostname == "SW-Nordvik-2").unwrap_or(false)
}

// ------------------------------------------------------------------ Vecka 2
fn su_w2(st: &mut State) {
    for l in st.links.iter_mut() {
        if l.a.dev == "SW1" && l.a.port == "GigabitEthernet0/6" {
            l.state = LinkState::Broken;
        }
    }
    both(st.dev_mut("SW2"), |c| {
        let i = iface(c, "GigabitEthernet0/24");
        i.speed = "100".into();
        i.duplex = "full".into();
    });
    both(st.dev_mut("SW1"), |c| iface(c, "GigabitEthernet0/7").access_vlan = 1);
}
pub(crate) fn ck_v2k1(st: &mut State) -> bool {
    let good = link_to_mut(st, "PC-Karim").map(|l| l.state == LinkState::Ok).unwrap_or(false);
    good && ok(st, "PC-Karim", "192.168.1.10")
}
pub(crate) fn ck_v2k2(st: &mut State) -> bool {
    let d = sim::get(st);
    match d.ports.get("SW2|GigabitEthernet0/24") {
        Some(p) => p.up && !p.mismatch,
        None => false,
    }
}
pub(crate) fn ck_v2n1(st: &mut State) -> bool { ok(st, "PC-Bo", "192.168.1.70") }

// ------------------------------------------------------------------ Vecka 3
fn su_w3(st: &mut State) {
    {
        let anna = st.dev_mut("PC-Anna");
        let nic = anna.nic.as_mut().unwrap();
        nic.dhcp = false;
        nic.static_ = Some(StaticIp {
            ip: "192.168.1.19".into(),
            mask: "255.255.255.0".into(),
            gw: Some("192.168.1.1".into()),
            dns: vec!["192.168.1.1".into()],
        });
    }
    sim::release(st, "PC-Anna");
    both(st.dev_mut("R1"), |c| c.dhcp_pools.get_mut("KONTOR").unwrap().mask = Some("255.255.255.248".into()));
    both(st.dev_mut("R1"), |c| c.dhcp_pools.get_mut("EKONOMI").unwrap().dns = vec![]);
}
pub(crate) fn ck_v3k1(st: &mut State) -> bool { ok(st, "PC-Anna", "192.168.1.70") && ok(st, "PC-Anna", "192.168.1.10") }
pub(crate) fn ck_v3k2(st: &mut State) -> bool {
    lease_in(st, "PC-Karim", "192.168.1.0", "255.255.255.192") && ok(st, "PC-Karim", "192.168.1.10")
}
pub(crate) fn ck_v3n1(st: &mut State) -> bool { sim::resolve(st, "PC-Maja", "ekonomi").error.is_none() }

// ------------------------------------------------------------------ Vecka 4
fn su_w4(st: &mut State) {
    both(st.dev_mut("SW1"), |c| {
        for n in ["GigabitEthernet0/24", "GigabitEthernet0/25"] {
            iface(c, n).allowed = Some(U::VlanList::List(vec![10, 30, 99]));
        }
    });
    both(st.dev_mut("SW2"), |c| {
        for n in ["GigabitEthernet0/24", "GigabitEthernet0/25"] {
            iface(c, n).native = 10;
        }
    });
}
pub(crate) fn ck_v4k1(st: &mut State) -> bool { ok(st, "PC-Bo", "192.168.1.70") }
pub(crate) fn ck_v4k2(st: &mut State) -> bool {
    let d = sim::get(st);
    d.native_mismatch.is_empty() && ok(st, "PC-Lisa", "192.168.1.11")
}
pub(crate) fn ck_v4n1(st: &mut State) -> bool { st.flags.misc.get("stpLooked").copied().unwrap_or(false) }

// ------------------------------------------------------------------ Vecka 5
fn su_w5(st: &mut State) {
    both(st.dev_mut("RB"), |c| c.routes.clear());
    both(st.dev_mut("R1"), |c| c.routes[0].nh = Some("203.0.113.9".into()));
    both(st.dev_mut("R1"), |c| iface(c, "GigabitEthernet0/0.20").dot1q = Some(21));
}
pub(crate) fn ck_v5k1(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") }
pub(crate) fn ck_v5k2(st: &mut State) -> bool { ok(st, "PC-Anna", "198.51.100.80") }
pub(crate) fn ck_v5n1(st: &mut State) -> bool { ok(st, "PC-Bo", "192.168.1.65") && ok(st, "PC-Bo", "192.168.1.10") }

// ------------------------------------------------------------------ Vecka 6
fn su_w6(st: &mut State) {
    both(st.dev_mut("R1"), |c| iface(c, "GigabitEthernet0/0.10").nat_dir = None);
    both(st.dev_mut("R1"), |c| {
        c.domain_name = None;
        c.crypto_key = None;
    });
    both(st.dev_mut("R1"), |c| c.lines.vty5_15 = Lines::vty("all", true));
}
pub(crate) fn ck_v6k1(st: &mut State) -> bool { ok(st, "PC-Anna", "198.51.100.80") }
pub(crate) fn ck_v6k2(st: &mut State) -> bool {
    let v = sim::ssh_enabled(st.dev("R1"));
    v.as_deref() == Some("2") && ok(st, "Tekniker", "192.168.1.193")
}
pub(crate) fn ck_v6n1(st: &mut State) -> bool { !sim::vty_allows(st.dev("R1"), "telnet") }

// ------------------------------------------------------------------ Vecka 7
fn su_w7(st: &mut State) {
    both(st.dev_mut("SW1"), |c| c.stp_off = vec![10]);
    both(st.dev_mut("SW2"), |c| c.stp_off = vec![10]);
    for l in st.links.iter_mut() {
        if l.b.dev == "PC-Karim" {
            l.state = LinkState::Flapping;
        }
    }
    both(st.dev_mut("SW2"), |c| c.ntp_servers = vec![]);
}
pub(crate) fn ck_v7k1(st: &mut State) -> bool {
    let d = sim::get(st);
    d.storm.is_empty() && !st.dev("SW1").cfg().stp_off.contains(&10) && !st.dev("SW2").cfg().stp_off.contains(&10)
}
pub(crate) fn ck_v7k2(st: &mut State) -> bool {
    link_to_mut(st, "PC-Karim").map(|l| l.state == LinkState::Ok).unwrap_or(false)
}
pub(crate) fn ck_v7n1(st: &mut State) -> bool { sim::ntp_synced(st, "SW2") }

// ------------------------------------------------------------------ Vecka 8
fn su_w8(st: &mut State) {
    both(st.dev_mut("SWB"), |c| iface(c, "GigabitEthernet0/1").poe = "never".into());
    let w = st.dev_mut("WLC").wlc.as_mut().unwrap();
    w.wlans.get_mut(&2).unwrap().iface = "lager".into();
    w.ap_channels.insert("AP-Lager-3".into(), 6);
    w.saved = Some(WlcSaved { wlans: w.wlans.clone(), ap_channels: w.ap_channels.clone() });
}
pub(crate) fn ck_v8k1(st: &mut State) -> bool {
    let d = sim::get(st);
    d.ap_joined.get("AP-Lager-1").copied().unwrap_or(false)
}
pub(crate) fn ck_v8k2(st: &mut State) -> bool {
    lease_in(st, "Gast-mobil", "192.168.2.64", "255.255.255.192")
}
pub(crate) fn ck_v8n1(st: &mut State) -> bool {
    let v: Vec<u32> = st.dev("WLC").wlc.as_ref().unwrap().ap_channels.values().copied().collect();
    v.len() == 3 && v[0] != v[1] && v[1] != v[2] && v[0] != v[2]
}

// ------------------------------------------------------------------ Vecka 9
fn su_w9(st: &mut State) {
    both(st.dev_mut("R1"), |c| {
        let i = iface(c, "GigabitEthernet0/0.10");
        i.acl_in = None;
        i.acl_out = Some("KONTOR-UT".into());
    });
    both(st.dev_mut("R1"), |c| c.acls.get_mut("GAST").unwrap().rules.retain(|r| r.seq != 30));
    // En gäst har kopplat in en egen liten switch i receptionen
    st.links.retain(|l| !(l.a.dev == "SW1" && l.a.port == "GigabitEthernet0/9"));
    st.devices.insert("Mini-switch".into(), Device::simple("Mini-switch", Kind::Hub, "gbg", "Okänd miniswitch"));
    let mut o = HostOpts::new("Okänd laptop");
    o.dhcp = true;
    st.devices.insert("Okand-laptop".into(), make_host("Okand-laptop", Os::Windows, o));
    st.links.push(Link::new("Lhub1", "SW1", "GigabitEthernet0/9", "Mini-switch", "p1", LinkKind::Copper, "blue"));
    st.links.push(Link::new("Lhub2", "Mini-switch", "p2", "Gast-laptop", "nic", LinkKind::Copper, "blue"));
    st.links.push(Link::new("Lhub3", "Mini-switch", "p3", "Okand-laptop", "nic", LinkKind::Copper, "purple"));
}
pub(crate) fn ck_v9k1(st: &mut State) -> bool {
    let inb = st.dev("R1").cfg().ifaces["GigabitEthernet0/0.10"].acl_in.as_deref() == Some("KONTOR-UT");
    !ok(st, "PC-Anna", "192.168.1.70") && ok(st, "PC-Anna", "198.51.100.80") && inb
}
pub(crate) fn ck_v9k2(st: &mut State) -> bool {
    if st.dev("R1").cfg().ifaces["GigabitEthernet0/0.30"].acl_in.as_deref() != Some("GAST") {
        return false;
    }
    let src = "192.168.1.140";
    let p = |dst: &str, proto: &str, dport: Option<i64>| AclPkt { src: src.into(), dst: dst.into(), proto: proto.into(), dport };
    sim::acl_eval(st, "R1", "GAST", &p("198.51.100.80", "icmp", None))
        && sim::acl_eval(st, "R1", "GAST", &p("9.9.9.9", "udp", Some(53)))
        && !sim::acl_eval(st, "R1", "GAST", &p("192.168.1.10", "tcp", Some(445)))
        && !sim::acl_eval(st, "R1", "GAST", &p("192.168.1.70", "tcp", Some(443)))
}
pub(crate) fn ck_v9n1(st: &mut State) -> bool {
    !st.dev("SW1").rt.errdisabled.contains_key("GigabitEthernet0/9")
        && !st.devices.contains_key("Mini-switch")
        && ok(st, "Gast-laptop", "198.51.100.80")
}
/// Vecka 9: ta bort den okända miniswitchen i receptionen
pub fn remove_mini_switch(st: &mut State) -> bool {
    if !st.devices.contains_key("Mini-switch") {
        return false;
    }
    st.links.retain(|l| l.a.dev != "Mini-switch" && l.b.dev != "Mini-switch");
    st.devices.shift_remove("Mini-switch");
    st.devices.shift_remove("Okand-laptop");
    st.links.push(Link::new("Lgast", "SW1", "GigabitEthernet0/9", "Gast-laptop", "nic", LinkKind::Copper, "blue"));
    sim::touch(st);
    true
}

// ------------------------------------------------------------------ Vecka 10
fn su_w10(st: &mut State) {
    // Fel 1 a: NAT-listan i Göteborg saknar undantaget för VPN-trafiken
    both(st.dev_mut("R1"), |c| c.acls.get_mut("NAT-UT").unwrap().rules.retain(|r| r.seq != 10));
    // Fel 1 b: Borås crypto-ACL speglar inte Göteborgs (/24 i stället för /26)
    both(st.dev_mut("RB"), |c| c.acls.get_mut("VPN-TRAFIK").unwrap().rules[0].dst = Some(AddrSpec::net("192.168.1.0", "0.0.0.255")));
    // Fel 2: ingen router klämmer MSS
    both(st.dev_mut("R1"), |c| iface(c, "GigabitEthernet0/1").adjust_mss = None);
    both(st.dev_mut("RB"), |c| iface(c, "GigabitEthernet0/1").adjust_mss = None);
    // Fel 3: webbtjänsten på Tid-2 har kraschat, men lastbalanseraren pingar bara
    st.dev_mut("Tid-2").services.clear();
    let lb = st.dev_mut("LB").lb.as_mut().unwrap();
    lb.pools.get_mut("TID-POOL").unwrap().monitor = "icmp".into();
    lb.saved = Some(lb.pools.clone());
}
pub(crate) fn ck_v10k1(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Anna", "192.168.2.1") }
pub(crate) fn ck_v10k2(st: &mut State) -> bool { sim::big_transfer(st, "PC-Lager", "192.168.1.10", 445).ok }
pub(crate) fn ck_v10n1(st: &mut State) -> bool {
    if st.devices.get("LB").and_then(|d| d.lb.as_ref()).is_none() {
        return false;
    }
    let stat = sim::lb_status(st, "LB");
    let Some(s) = stat.first() else { return false };
    if s.pool.monitor == "icmp" {
        return false;
    }
    let up: Vec<_> = s.members.iter().filter(|m| m.up).collect();
    if up.is_empty() {
        return false;
    }
    for m in up {
        if !sim::ping(st, "LB", &m.m.ip, PingOpts::tcp(m.m.port)).ok {
            return false;
        }
    }
    true
}

// ------------------------------------------------------------------ Veckoutmaningar
/// Det spelet vet om spelarens vecka när en utmaning avgörs
pub struct ChallengeCtx<'a> {
    /// (enhet, kommandorad) i den ordning de skrevs
    pub cmd_log: &'a [(String, String)],
    pub commands: usize,
    /// Någon ledtråd har öppnats
    pub any_hints: bool,
    pub elapsed_ms: f64,
    /// Spelaren tittade på övervakningen innan racket (flaggan monitorFirst)
    pub monitor_first: bool,
    /// Alla felrapporter fick full poäng
    pub all_reports_perfect: bool,
}

/// Utmaningens titel (None om veckan saknar utmaning)
pub fn challenge_title(week: u32) -> Option<&'static str> {
    def(week).and_then(|w| w.challenge)
}
/// "need": utmaningen går inte att avgöra förrän veckan är slut (kommandon som måste användas)
pub fn challenge_need(week: u32) -> bool {
    matches!(week, 4 | 5 | 9..=14)
}

fn is_word(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_')
}
/// Matchar `^(do\s+)?sh\w*\s+<p1>\s+<p2>...` där (true, s) = exakt ord och (false, s) = ord som börjar på s
fn show_match(line: &str, pats: &[(bool, &str)], anchored: bool) -> bool {
    let low = line.trim().to_lowercase();
    let mut toks: Vec<&str> = low.split_whitespace().collect();
    if toks.first() == Some(&"do") && toks.len() > 1 {
        toks.remove(0);
    }
    if toks.is_empty() || !toks[0].starts_with("sh") || !is_word(toks[0]) {
        return false;
    }
    let rest = &toks[1..];
    if rest.len() < pats.len() || (anchored && rest.len() != pats.len()) {
        return false;
    }
    pats.iter().zip(rest).all(|((exact, p), t)| if *exact { t == p } else { t.starts_with(p) })
}
fn first_word(line: &str) -> String {
    line.trim().to_lowercase().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}
fn is_trace(line: &str) -> bool {
    matches!(first_word(line).as_str(), "traceroute" | "tracert" | "trace")
}
fn is_df_ping(line: &str) -> bool {
    let low = line.trim().to_lowercase();
    if first_word(&low) != "ping" {
        return false;
    }
    let toks: Vec<&str> = low.split_whitespace().skip(1).collect();
    toks.iter().enumerate().any(|(i, t)| {
        *t == "-f" || *t == "df-bit" || (*t == "-m" && toks.get(i + 1).map(|n| *n == "do").unwrap_or(false))
    })
}
fn used(g: &ChallengeCtx, f: impl Fn(&str) -> bool) -> bool {
    g.cmd_log.iter().any(|(_, l)| f(l))
}
fn trace_devs<'a>(g: &'a ChallengeCtx) -> Vec<&'a str> {
    let mut v: Vec<&str> = vec![];
    for (d, l) in g.cmd_log {
        if is_trace(l) && !v.contains(&d.as_str()) {
            v.push(d);
        }
    }
    v
}
/// Har spelaren klarat veckans utmaning?
pub fn challenge_done(week: u32, g: &ChallengeCtx) -> bool {
    match week {
        1 => !used(g, |l| show_match(l, &[(false, "run")], false)),
        2 => g.commands <= 40,
        3 => !g.any_hints,
        4 => used(g, |l| show_match(l, &[(false, "int"), (false, "tr")], false)),
        5 => used(g, is_trace),
        6 => g.elapsed_ms <= 15.0 * 60000.0,
        7 => g.monitor_first,
        8 => g.all_reports_perfect,
        9 => used(g, |l| show_match(l, &[(false, "acc")], false)),
        10 => used(g, |l| show_match(l, &[(false, "cry"), (false, "ips")], false)) && used(g, is_df_ping),
        11 => trace_devs(g).len() >= 2,
        12 => {
            let devs: Vec<&str> = g
                .cmd_log
                .iter()
                .filter(|(_, l)| show_match(l, &[(false, "cry"), (false, "isa"), (false, "pol")], false))
                .map(|(d, _)| d.as_str())
                .collect();
            devs.contains(&"R1") && devs.contains(&"RB")
        }
        13 => used(g, |l| show_match(l, &[(true, "ip"), (true, "ospf"), (false, "int"), (false, "br")], false)),
        14 => {
            used(g, |l| show_match(l, &[(true, "ip"), (true, "bgp")], true))
                && used(g, |l| show_match(l, &[(true, "ip"), (true, "bgp"), (false, "sum")], false))
        }
        _ => false,
    }
}

/// Svårt läge: hur spelaren bevisar att ett fel är löst.
/// Felet räknas som löst först när check() är sant OCH spelaren därefter har kört ett av
/// kommandona i `cmds` på en av enheterna i `on` (efter att felet rättades).
#[derive(Clone, Debug)]
pub struct Verify {
    /// Enheter (id) där beviset ska köras, t.ex. ["PC-Anna"] eller ["SW1", "Tekniker"]
    pub on: Vec<&'static str>,
    /// Godkända kommandon som prefix (gemener, förkortningar tillåts av UI:t), t.ex. ["ping 192.168.1.10", "ping filserver"]
    pub cmds: Vec<&'static str>,
    /// Text i ärendelistan, t.ex. "Bevisa: pinga filservern (192.168.1.10) från Annas dator."
    pub text: &'static str,
}

/// Beviset för en uppgift i svårt läge (byggsteg och "titta"-uppgifter får också ett).
pub fn verify(task_id: &str) -> Verify {
    verify::table(task_id).unwrap_or_else(|| Verify {
        on: vec!["Tekniker"],
        cmds: vec!["ping"],
        text: "Bevisa att felet är löst med ett ping från laptopen.",
    })
}

/// Matchar en inskriven rad mot ett godkänt bevis (skiftlägesokänsligt, tål extra mellanslag
/// och IOS-förkortningar som "sh int status" för "show interfaces status").
pub fn verify_matches(v: &Verify, dev: &str, line: &str) -> bool {
    verify::matches(v, dev, line)
}

#[path = "levels_verify.rs"]
mod verify;
