//! Nätverkssimulatorn. Allt härleds ur konfigurationen och den fysiska kopplingen,
//! så att ett fel bara försvinner när orsaken faktiskt är rättad.
//! Port av js/net/sim.js. Typerna och signaturerna här är det gemensamma API:t för resten av spelet.
#![allow(unused_variables, dead_code)]
use super::dynroute::{self, DynOut};
use super::model::*;
use super::util::{self as U, Rng};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::Rc;

pub const TUNNEL_MTU: i64 = 1420;

/// Kända adresser på internet (INTERNET i sim.js)
pub fn internet_host(ip: &str) -> Option<(&'static str, bool, bool)> {
    // (namn, dns, web)
    match ip {
        "9.9.9.9" => Some(("dns.quad9.net", true, false)),
        "198.51.100.80" => Some(("www.example.com", false, true)),
        "198.51.100.25" => Some(("mail.example.com", false, false)),
        _ => None,
    }
}
pub fn public_dns(name: &str) -> Option<&'static str> {
    match name {
        "www.example.com" | "example.com" => Some("198.51.100.80"),
        "dns.quad9.net" => Some("9.9.9.9"),
        "mail.example.com" => Some("198.51.100.25"),
        "www.nordvik.example" => Some("203.0.113.11"),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Flags {
    /// Extra PoE-last per switch (state.flags.poeExtra)
    pub poe_extra: HashMap<String, f64>,
    /// Övriga flaggor som spelet sätter (t.ex. "stpLooked")
    pub misc: HashMap<String, bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DynMem {
    pub ospf: HashMap<String, bool>,
    pub bgp: HashMap<String, String>,
    pub since: HashMap<String, f64>,
    pub ready: bool,
}

/// Hela spelvärldens nät (state i JS).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub devices: IndexMap<String, Device>,
    pub links: Vec<Link>,
    pub time: f64,
    pub flags: Flags,
    pub rng: Rng,
    pub dyn_mem: DynMem,
    /// Senaste portstatus (för loggrader vid ändring), nyckel "dev|port"
    pub port_snap: HashMap<String, String>,
    #[serde(skip)]
    pub cache: Option<Rc<Derived>>,
    #[serde(skip)]
    pub dirty: bool,
    /// Tyst beräkning (inga OSPF/BGP-loggrader), t.ex. när en kontroll provar något tillfälligt
    #[serde(skip)]
    pub quiet: bool,
    /// Cache för NTP-synk per enhet, töms vid omberäkning
    #[serde(skip)]
    pub ntp_cache: HashMap<String, bool>,
    /// Nya loggrader (enhet, rad) som terminalen kan visa med "terminal monitor"/konsol. Töms av UI.
    #[serde(skip)]
    pub log_events: Vec<(String, String)>,
}

impl State {
    pub fn new(t: Topology) -> State {
        State {
            devices: t.devices,
            links: t.links,
            time: 0.0,
            flags: Flags::default(),
            rng: Rng::new(0x9e3779b97f4a7c15),
            dyn_mem: DynMem::default(),
            port_snap: HashMap::new(),
            cache: None,
            dirty: true,
            quiet: false,
            ntp_cache: HashMap::new(),
            log_events: vec![],
        }
    }
    pub fn dev(&self, id: &str) -> &Device {
        &self.devices[id]
    }
    pub fn dev_mut(&mut self, id: &str) -> &mut Device {
        self.devices.get_mut(id).expect("enhet finns")
    }
    pub fn random(&mut self) -> f64 {
        self.rng.next_f64()
    }
}

// ------------------------------------------------------------------ Härledd data (D i JS)

#[derive(Clone, Debug, PartialEq)]
pub struct Neg {
    pub speed: u32,
    pub duplex: String,
    pub auto: bool,
}

#[derive(Clone, Debug)]
pub struct PortInfo {
    pub link: String, // länkens id
    pub peer: End,
    pub up: bool,
    pub admin_up: bool,
    pub neg: Option<Neg>,
    pub mismatch: bool,
}

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub up: bool,
    pub mismatch: bool,
    pub flapping: bool,
    pub neg_a: Option<Neg>,
    pub neg_b: Option<Neg>,
}

#[derive(Clone, Debug)]
pub struct PoePort {
    pub admin: String, // "auto" | "off"
    pub oper: String,  // "on" | "off" | "faulty"
    pub watts: f64,
    pub device: String,
    pub cls: String,
}

#[derive(Clone, Debug)]
pub struct PoeInfo {
    pub available: f64,
    pub used: f64,
    pub remaining: f64,
    pub ports: IndexMap<String, PoePort>,
}

#[derive(Clone, Debug)]
pub struct NativeMismatch {
    pub link: String,
    pub a: End,
    pub b: End,
    pub na: u16,
    pub nb: u16,
}

#[derive(Clone, Debug)]
pub struct StpInfo {
    pub root: String,
    pub is_root: bool,
    pub root_port: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub to: String,
    /// Länkens id, "wifi" för trådlöst, None för SVI-kanter
    pub link: Option<String>,
    pub lossy: f64,
}

/// En adress i nätet (endpoint). key = "E:dev:iface"
#[derive(Clone, Debug)]
pub struct Ep {
    pub key: String,
    pub dev: String,
    pub iface: String,
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub gw: Option<String>,
    pub mac: String,
    pub up: bool,
    pub is_loop: bool,
}

#[derive(Clone, Debug)]
pub struct WifiInfo {
    pub ap: String,
    pub vlan: u16,
    pub ssid: String,
    pub interference: bool,
    pub aps: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Derived {
    /// nyckel key(dev, port) = "dev|port"
    pub ports: HashMap<String, PortInfo>,
    /// nyckel länkens id
    pub links: HashMap<String, LinkInfo>,
    pub poe: IndexMap<String, PoeInfo>,
    /// nyckel "dev|port#vlan" -> "BLK" eller "BKN*"
    pub blocked: HashMap<String, String>,
    pub native_mismatch: Vec<NativeMismatch>,
    pub stp: HashMap<String, BTreeMap<u16, StpInfo>>,
    /// VLAN -> switchar i stormen
    pub storm: BTreeMap<u16, Vec<String>>,
    pub ap_joined: IndexMap<String, bool>,
    pub sec_blocked: HashSet<String>,
    pub wifi: IndexMap<String, WifiInfo>,
    /// L2-grannar per nod ("S:dev:vlan", "E:dev:iface", "W:ap:vlan", "H:hub")
    pub adj: IndexMap<String, Vec<Edge>>,
    /// Segment (sammanhängande komponent) per nod
    pub comp: HashMap<String, usize>,
    pub eps: IndexMap<String, Ep>,
    /// Segment -> ep-nycklar i ordning
    pub by_seg: HashMap<usize, Vec<String>>,
    pub dynr: DynOut,
}

impl Derived {
    pub fn ep(&self, dev: &str, iface: &str) -> Option<&Ep> {
        self.eps.get(&format!("E:{}:{}", dev, iface))
    }
    pub fn port(&self, dev: &str, port: &str) -> Option<&PortInfo> {
        self.ports.get(&key(dev, port))
    }
    pub fn seg_eps(&self, seg: usize) -> Vec<&Ep> {
        self.by_seg.get(&seg).map(|v| v.iter().filter_map(|k| self.eps.get(k)).collect()).unwrap_or_default()
    }
}

// ------------------------------------------------------------------ Resultattyper

#[derive(Clone, Debug)]
pub struct Hop {
    pub dev: String,
    pub ip: String,
}

#[derive(Clone, Debug, Default)]
pub struct PingOpts {
    pub src: Option<String>,
    /// "icmp" (standard), "tcp" eller "udp"
    pub proto: Option<String>,
    pub dport: Option<i64>,
    pub learn_arp: bool,
    pub size: Option<i64>,
    pub df: bool,
}

impl PingOpts {
    pub fn tcp(port: i64) -> Self {
        PingOpts { proto: Some("tcp".into()), dport: Some(port), ..Default::default() }
    }
    pub fn udp(port: i64) -> Self {
        PingOpts { proto: Some("udp".into()), dport: Some(port), ..Default::default() }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pkt {
    pub src: Option<String>,
    pub dst: String,
    pub proto: String,
    pub dport: Option<i64>,
    pub ttl: i64,
    pub count: Option<f64>,
    pub nat_local: Option<String>,
    pub mss: Option<i64>,
    pub tunnel: bool,
}

/// Resultatet av send(): vart paketet tog vägen.
#[derive(Clone, Debug, Default)]
pub struct SendResult {
    pub ok: bool,
    /// Enhet där paketet kom fram, eller "INTERNET:<ip>"
    pub at: Option<String>,
    pub pkt: Pkt,
    pub hops: Vec<Hop>,
    pub loss: f64,
    /// "drop", "acl", "net-unreachable", "ttl", "ipsec", "frag", "nolink", "noip", "nogw", "arp"
    pub reason: Option<String>,
    pub where_: Option<String>,
    pub iface: Option<String>,
    pub from_ip: Option<String>,
    pub nh: Option<String>,
    pub local: bool,
    pub mtu: Option<i64>,
    pub ipsec: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PingResult {
    pub ok: bool,
    /// Som SendResult.reason, plus "refused" och "noreply"
    pub reason: Option<String>,
    pub loss: f64,
    pub hops: Vec<Hop>,
    pub at: Option<String>,
    pub ttl: i64,
    pub reply_from: Option<String>,
    pub src_used: Option<String>,
    pub tunnel: bool,
    pub mss: i64,
    pub where_: Option<String>,
    pub from_ip: Option<String>,
    pub local: bool,
    pub iface: Option<String>,
    pub mtu: Option<i64>,
    pub ipsec: Option<String>,
    /// Vid "noreply": varför svaret inte kom hem
    pub back_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IpConf {
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub gw: Option<String>,
    pub dns: Vec<String>,
    pub apipa: bool,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub ip: Option<String>,
    /// "nolookup", "noserver", "nxdomain", "servfail", "timeout"
    pub error: Option<String>,
    pub server: Option<String>,
}

/// En rutt som routingvalet hittade (routeLookup)
#[derive(Clone, Debug)]
pub struct RouteHit {
    /// "C", "S", "O", "O*E2", "B"
    pub kind: String,
    pub net: String,
    pub mask: String,
    pub nh: Option<String>,
    pub iface: String,
    pub ep_key: String,
    pub ad: i64,
    pub metric: i64,
    pub direct: bool,
    pub prefix: u32,
}

/// En rad i routingtabellen (installedRoutes)
#[derive(Clone, Debug)]
pub struct InstalledRoute {
    /// "C", "L", "S", "S*", "O", "O*E2", "B"
    pub kind: String,
    pub net: String,
    pub mask: String,
    pub nh: Option<String>,
    pub iface: Option<String>,
    pub ad: i64,
    pub metric: i64,
    pub path: Vec<i64>,
}

#[derive(Clone, Debug)]
pub struct Clock {
    pub synced: bool,
    pub ms: f64,
    pub mon: &'static str,
    pub day: u32,
    pub dow: &'static str,
    pub year: i64,
    pub hms: String,
    pub msec: String,
}

/// Paket för ACL-kontroller (aclEval)
#[derive(Clone, Debug, Default)]
pub struct AclPkt {
    pub src: String,
    pub dst: String,
    pub proto: String,
    pub dport: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct IsakmpStatus {
    pub dst: String,
    pub src: String,
    pub state: String,
    pub conn_id: u32,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct SaStatus {
    pub iface: String,
    pub map: String,
    pub seq: i64,
    pub local: String,
    pub peer: String,
    pub rule: Ace,
    pub up: bool,
    pub encaps: f64,
    pub decaps: f64,
    pub spi: u32,
    pub mtu: i64,
}

#[derive(Clone, Debug, Default)]
pub struct CryptoStatus {
    pub isakmp: Vec<IsakmpStatus>,
    pub sas: Vec<SaStatus>,
}

#[derive(Clone, Debug)]
pub struct BigTransfer {
    pub ok: bool,
    /// "connect" eller "transfer"
    pub stage: Option<String>,
    pub res: PingResult,
    pub mtu: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct HttpResult {
    pub ok: bool,
    pub code: i64,
    pub server: Option<String>,
    pub via: Option<String>,
    pub member: Option<LbMember>,
    pub res: PingResult,
}

#[derive(Clone, Debug)]
pub struct LbMemberStatus {
    pub m: LbMember,
    pub up: bool,
    pub req: i64,
    pub fail: i64,
}

#[derive(Clone, Debug)]
pub struct LbPoolStatus {
    pub name: String,
    pub pool: LbPool,
    pub members: Vec<LbMemberStatus>,
}

#[derive(Clone, Debug)]
pub struct BridgeId {
    pub pr: u32,
    pub mac: String,
}

// ------------------------------------------------------------------ Små hjälpfunktioner

pub fn key(dev: &str, port: &str) -> String {
    format!("{}|{}", dev, port)
}

pub fn port_num(name: &str) -> u32 {
    let digits: String = name.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().unwrap_or(0)
}

// ------------------------------------------------------------------ API (porten av sim.js)

fn is_switch(d: &Device) -> bool {
    d.kind == Kind::Switch
}
fn is_router(d: &Device) -> bool {
    d.kind == Kind::Router
}
fn ek(dev: &str, iface: &str) -> String {
    format!("E:{}:{}", dev, iface)
}

/// Kabeln som sitter i en port: (länk, motsatt ände)
pub fn link_at<'a>(st: &'a State, dev: &str, port: &str) -> Option<(&'a Link, &'a End)> {
    for l in st.links.iter() {
        if l.a.dev == dev && l.a.port == port {
            return Some((l, &l.b));
        }
        if l.b.dev == dev && l.b.port == port {
            return Some((l, &l.a));
        }
    }
    None
}

fn find_host_link<'a>(st: &'a State, id: &str) -> Option<&'a End> {
    for l in st.links.iter() {
        if l.a.dev == id {
            return Some(&l.b);
        }
        if l.b.dev == id {
            return Some(&l.a);
        }
    }
    None
}

// ---------------------------------------------------------------- Lager 1

fn side_admin_up(st: &State, dev: &str, port: &str) -> bool {
    let d = match st.devices.get(dev) {
        Some(d) if d.powered => d,
        _ => return false,
    };
    if d.os == Os::Ios {
        let i = match d.ifc(port) {
            Some(i) if !i.shutdown => i,
            _ => return false,
        };
        if d.rt.errdisabled.get(port).map(|s| !s.is_empty()).unwrap_or(false) {
            return false;
        }
        let _ = i;
        return true;
    }
    if d.kind == Kind::Hub {
        return true;
    }
    if let Some(n) = &d.nic {
        return n.enabled;
    }
    true
}

struct SideNeg {
    speed: String,
    duplex: String,
    max: i64,
}

fn side_neg(st: &State, dev: &str, port: &str, kind: LinkKind) -> SideNeg {
    let auto = || SideNeg { speed: "auto".into(), duplex: "auto".into(), max: 1000 };
    if kind == LinkKind::Fiber || kind == LinkKind::Wan {
        return auto();
    }
    if let Some(d) = st.devices.get(dev) {
        if d.os == Os::Ios {
            if let Some(i) = d.ifc(port) {
                let sp = if i.speed.is_empty() { "auto".to_string() } else { i.speed.clone() };
                let du = if i.duplex.is_empty() { "auto".to_string() } else { i.duplex.clone() };
                return SideNeg { speed: sp, duplex: du, max: 1000 };
            }
        }
    }
    auto()
}

fn parse_int(s: &str) -> i64 {
    let t: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    t.parse().unwrap_or(0)
}

fn negotiate(a: &SideNeg, b: &SideNeg) -> Option<(Neg, Neg)> {
    let auto_a = a.speed == "auto" && a.duplex == "auto";
    let auto_b = b.speed == "auto" && b.duplex == "auto";
    let sp = |x: &SideNeg| -> i64 { if x.speed == "auto" { a.max.min(b.max) } else { parse_int(&x.speed) } };
    let dp = |x: &SideNeg| -> String { if x.duplex == "auto" { "full".into() } else { x.duplex.clone() } };
    if auto_a && auto_b {
        let s = a.max.min(b.max) as u32;
        let n = Neg { speed: s, duplex: "full".into(), auto: true };
        return Some((n.clone(), n));
    }
    if auto_a != auto_b {
        let fixed = if auto_a { b } else { a };
        let s = sp(fixed);
        let fx = Neg { speed: s as u32, duplex: dp(fixed), auto: false };
        let au = Neg { speed: s as u32, duplex: if s >= 1000 { "full".into() } else { "half".into() }, auto: true };
        return Some(if auto_a { (au, fx) } else { (fx, au) });
    }
    let (sa, sb) = (sp(a), sp(b));
    if sa != sb {
        return None;
    }
    Some((Neg { speed: sa as u32, duplex: dp(a), auto: false }, Neg { speed: sb as u32, duplex: dp(b), auto: false }))
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// PoE: vilka enheter får ström
fn compute_poe(st: &mut State, d: &mut Derived) {
    let ids: Vec<String> = st.devices.keys().cloned().collect();
    for id in &ids {
        let sw = st.dev(id);
        if !is_switch(sw) {
            continue;
        }
        let extra = st.flags.poe_extra.get(id).copied().unwrap_or(0.0);
        let mut used = 0.0;
        let mut info = PoeInfo { available: sw.poe_budget, used: 0.0, remaining: sw.poe_budget, ports: IndexMap::new() };
        let mut names: Vec<&String> = sw
            .cfg()
            .ifaces
            .keys()
            .filter(|n| n.starts_with("GigabitEthernet0/") && n["GigabitEthernet0/".len()..].chars().all(|c| c.is_ascii_digit()) && n.len() > "GigabitEthernet0/".len() && port_num(n) <= 24)
            .collect();
        names.sort_by_key(|n| port_num(n));
        for n in names {
            let i = &sw.cfg().ifaces[n];
            let la = link_at(st, id, n);
            let pd = la.and_then(|(_, peer)| st.devices.get(&peer.dev));
            let mut p = PoePort { admin: if i.poe == "never" { "off".into() } else { "auto".into() }, oper: "off".into(), watts: 0.0, device: "n/a".into(), cls: "n/a".into() };
            if let (Some((l, _)), Some(pd)) = (la, pd) {
                if pd.poe_watts != 0.0 && l.state != LinkState::Unplugged && l.state != LinkState::Broken && !pd.external_power {
                    if sw.poe_budget > 0.0 && i.poe != "never" && !i.shutdown && sw.powered {
                        if sw.poe_budget - used - extra >= pd.poe_watts {
                            used += pd.poe_watts;
                            p.oper = "on".into();
                            p.watts = pd.poe_watts;
                            p.device = "AIR-CAP3702I-E-K9".into();
                            p.cls = pd.poe_class.to_string();
                        } else {
                            p.oper = "faulty".into();
                            p.device = "n/a".into();
                        }
                    }
                }
            }
            info.ports.insert(n.clone(), p);
        }
        info.used = round1(used + extra);
        info.remaining = round1(sw.poe_budget - used - extra);
        d.poe.insert(id.clone(), info);
    }
    // Ström till PoE-enheter
    for id in &ids {
        let dev = st.dev(id);
        if dev.poe_watts == 0.0 {
            continue;
        }
        if dev.external_power {
            st.dev_mut(id).powered = true;
            continue;
        }
        let on = match find_host_link(st, id) {
            Some(peer) => d.poe.get(&peer.dev).and_then(|p| p.ports.get(&peer.port)).map(|p| p.oper == "on").unwrap_or(false),
            None => false,
        };
        st.dev_mut(id).powered = on;
    }
}

fn physical(st: &State, d: &mut Derived) {
    for l in st.links.iter() {
        let up_a = side_admin_up(st, &l.a.dev, &l.a.port);
        let up_b = side_admin_up(st, &l.b.dev, &l.b.port);
        let cable = l.state == LinkState::Ok || l.state == LinkState::Flapping;
        let neg = negotiate(&side_neg(st, &l.a.dev, &l.a.port, l.kind), &side_neg(st, &l.b.dev, &l.b.port, l.kind));
        let up = up_a && up_b && cable && neg.is_some();
        let mismatch = match &neg {
            Some((a, b)) => up && a.duplex != b.duplex,
            None => false,
        };
        let (na, nb) = match neg {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        };
        d.links.insert(l.id.clone(), LinkInfo { up, mismatch, flapping: up && l.state == LinkState::Flapping, neg_a: na.clone(), neg_b: nb.clone() });
        d.ports.insert(key(&l.a.dev, &l.a.port), PortInfo { link: l.id.clone(), peer: l.b.clone(), up, admin_up: up_a, neg: na, mismatch });
        d.ports.insert(key(&l.b.dev, &l.b.port), PortInfo { link: l.id.clone(), peer: l.a.clone(), up, admin_up: up_b, neg: nb, mismatch });
    }
}

// ---------------------------------------------------------------- Lager 2

/// "access" eller "trunk"
pub fn op_mode(st: &State, dev: &str, port: &str) -> &'static str {
    let d = match st.devices.get(dev) {
        Some(d) => d,
        None => return "access",
    };
    let i = match d.ifc(port) {
        Some(i) => i,
        None => return "access",
    };
    if i.mode == "access" {
        return "access";
    }
    if i.mode == "trunk" {
        return "trunk";
    }
    let (_, peer) = match link_at(st, dev, port) {
        Some(x) => x,
        None => return "access",
    };
    let pd = match st.devices.get(&peer.dev) {
        Some(p) if is_switch(p) => p,
        _ => return "access",
    };
    let pi = match pd.ifc(&peer.port) {
        Some(p) => p,
        None => return "access",
    };
    if i.nonegotiate || pi.nonegotiate {
        return if pi.mode == "trunk" && i.mode == "trunk" { "trunk" } else { "access" };
    }
    let (me, other) = (i.mode.as_str(), pi.mode.as_str());
    if other == "trunk" && me != "access" {
        return "trunk";
    }
    if other == "dynamic desirable" && (me == "dynamic auto" || me == "dynamic desirable") {
        return "trunk";
    }
    if me == "dynamic desirable" && other == "dynamic auto" {
        return "trunk";
    }
    "access"
}

pub fn trunk_vlans(sw: &Device, i: &Iface) -> Vec<u16> {
    let mut list: Vec<u16> = if i.allowed_all() { sw.cfg().vlans.keys().copied().collect() } else { i.allowed_list() };
    list.retain(|v| vlan_exists(sw, *v));
    list.sort();
    list
}

fn carries_vlan(st: &State, dev: &str, port: &str, v: u16) -> bool {
    let d = st.dev(dev);
    let i = match d.ifc(port) {
        Some(i) => i,
        None => return false,
    };
    if op_mode(st, dev, port) == "access" {
        return i.access_vlan == v && vlan_exists(d, v);
    }
    trunk_vlans(d, i).contains(&v)
}

fn bid_less(a: &BridgeId, b: &BridgeId) -> bool {
    a.pr < b.pr || (a.pr == b.pr && a.mac < b.mac)
}

/// STP och native-mismatch: räknar ut blockerade (port, vlan)
fn spanning_tree(st: &State, d: &mut Derived) {
    let sw_links: Vec<&Link> = st
        .links
        .iter()
        .filter(|l| {
            let a = st.devices.get(&l.a.dev);
            let b = st.devices.get(&l.b.dev);
            matches!((a, b), (Some(a), Some(b)) if is_switch(a) && is_switch(b)) && d.links[&l.id].up
        })
        .collect();
    // native mismatch
    for l in &sw_links {
        let ma = op_mode(st, &l.a.dev, &l.a.port);
        let mb = op_mode(st, &l.b.dev, &l.b.port);
        if ma == "trunk" && mb == "trunk" {
            let na = st.dev(&l.a.dev).ifc(&l.a.port).map(|i| i.native).unwrap_or(1);
            let nb = st.dev(&l.b.dev).ifc(&l.b.port).map(|i| i.native).unwrap_or(1);
            if na != nb {
                d.native_mismatch.push(NativeMismatch { link: l.id.clone(), a: l.a.clone(), b: l.b.clone(), na, nb });
                for s in [&l.a, &l.b] {
                    d.blocked.insert(format!("{}#{}", key(&s.dev, &s.port), na), "BKN*".into());
                    d.blocked.insert(format!("{}#{}", key(&s.dev, &s.port), nb), "BKN*".into());
                }
            }
        }
    }
    // alla VLAN som finns på någon switch
    let mut vset: BTreeSet<u16> = BTreeSet::new();
    for dev in st.devices.values() {
        if is_switch(dev) {
            vset.extend(dev.cfg().vlans.keys().copied());
        }
    }
    for v in vset {
        let carrying: Vec<&&Link> = sw_links
            .iter()
            .filter(|l| {
                carries_vlan(st, &l.a.dev, &l.a.port, v)
                    && carries_vlan(st, &l.b.dev, &l.b.port, v)
                    && !d.blocked.contains_key(&format!("{}#{}", key(&l.a.dev, &l.a.port), v))
            })
            .collect();
        // Rotbrygga per sammanhängande grupp (här räcker enkel jämförelse)
        let mut groups: IndexMap<String, Vec<&Link>> = IndexMap::new();
        for l in carrying {
            let mut pair = [l.a.dev.clone(), l.b.dev.clone()];
            pair.sort();
            groups.entry(pair.join("+")).or_default().push(l);
        }
        for (_, ls) in groups {
            let s1 = st.dev(&ls[0].a.dev);
            let s2 = st.dev(&ls[0].b.dev);
            let on1 = !s1.cfg().stp_off.contains(&v);
            let on2 = !s2.cfg().stp_off.contains(&v);
            let root = if bid_less(&bridge_id(s1, v), &bridge_id(s2, v)) { s1 } else { s2 };
            let non = if root.id == s1.id { s2 } else { s1 };
            d.stp.entry(root.id.clone()).or_default();
            d.stp.entry(non.id.clone()).or_default();
            d.stp.get_mut(&root.id).unwrap().insert(v, StpInfo { root: root.id.clone(), is_root: true, root_port: None });
            d.stp.get_mut(&non.id).unwrap().insert(v, StpInfo { root: root.id.clone(), is_root: false, root_port: None });
            if ls.len() < 2 {
                let l0 = ls[0];
                let np = if l0.a.dev == non.id { l0.a.port.clone() } else { l0.b.port.clone() };
                d.stp.get_mut(&non.id).unwrap().get_mut(&v).unwrap().root_port = Some(np);
                continue;
            }
            if !on1 && !on2 {
                let e = d.storm.entry(v).or_default();
                e.push(s1.id.clone());
                e.push(s2.id.clone());
                continue;
            }
            let mut sides: Vec<String> = ls.iter().map(|l| if l.a.dev == non.id { l.a.port.clone() } else { l.b.port.clone() }).collect();
            sides.sort_by_key(|p| port_num(p));
            d.stp.get_mut(&non.id).unwrap().get_mut(&v).unwrap().root_port = Some(sides[0].clone());
            for s in sides.iter().skip(1) {
                d.blocked.insert(format!("{}#{}", key(&non.id, s), v), "BLK".into());
            }
        }
    }
}

fn blocked_node(d: &Derived, n: &str) -> bool {
    if let Some(rest) = n.strip_prefix("E:") {
        if let Some(dev) = rest.strip_suffix(":nic") {
            if !dev.is_empty() {
                return d.sec_blocked.contains(dev);
            }
        }
    }
    false
}

fn add_edge(d: &mut Derived, a: &str, b: &str, link: Option<&str>, lossy: f64) {
    d.adj.entry(a.to_string()).or_default().push(Edge { to: b.to_string(), link: link.map(|s| s.to_string()), lossy });
    d.adj.entry(b.to_string()).or_default().push(Edge { to: a.to_string(), link: link.map(|s| s.to_string()), lossy });
}

/// Vad en port lämnar ut på kabeln: kanal -> nod
fn port_channels(st: &State, d: &Derived, dev: &str, port: &str, pass: u8) -> IndexMap<String, String> {
    let mut ch: IndexMap<String, String> = IndexMap::new();
    let dd = match st.devices.get(dev) {
        Some(x) => x,
        None => return ch,
    };
    if is_switch(dd) {
        let i = match dd.ifc(port) {
            Some(i) => i,
            None => return ch,
        };
        let m = op_mode(st, dev, port);
        let k = key(dev, port);
        if m == "access" {
            if vlan_exists(dd, i.access_vlan) && !d.blocked.contains_key(&format!("{}#{}", k, i.access_vlan)) {
                ch.insert("u".into(), format!("S:{}:{}", dev, i.access_vlan));
            }
        } else {
            for v in trunk_vlans(dd, i) {
                if d.blocked.contains_key(&format!("{}#{}", k, v)) {
                    continue;
                }
                if v == i.native {
                    ch.insert("u".into(), format!("S:{}:{}", dev, v));
                } else {
                    ch.insert(format!("t{}", v), format!("S:{}:{}", dev, v));
                }
            }
            if d.native_mismatch.iter().any(|nm| (nm.a.dev == dev && nm.a.port == port) || (nm.b.dev == dev && nm.b.port == port)) {
                ch.shift_remove("u");
            }
        }
        return ch;
    }
    if is_router(dd) {
        if dd.ifc(port).map(|p| p.ip.is_some()).unwrap_or(false) {
            ch.insert("u".into(), ek(dev, port));
        }
        for (n, s) in dd.cfg().ifaces.iter() {
            if s.parent.as_deref() == Some(port) && s.dot1q.is_some() && !s.shutdown {
                ch.insert(format!("t{}", s.dot1q.unwrap()), ek(dev, n));
            }
        }
        return ch;
    }
    if dd.kind == Kind::Hub {
        ch.insert("u".into(), format!("H:{}", dev));
        return ch;
    }
    if dd.kind == Kind::Ap {
        ch.insert("u".into(), ek(dev, "nic"));
        if pass == 2 && d.ap_joined.get(dev).copied().unwrap_or(false) {
            if let Some(w) = st.devices.get("WLC").and_then(|w| w.wlc.as_ref()) {
                for wl in w.wlans.values() {
                    if !wl.enabled {
                        continue;
                    }
                    if let Some(f) = w.interfaces.get(&wl.iface) {
                        ch.insert(format!("t{}", f.vlan), format!("W:{}:{}", dev, f.vlan));
                    }
                }
            }
        }
        return ch;
    }
    ch.insert("u".into(), ek(dev, "nic"));
    ch
}

fn build_l2(st: &State, d: &mut Derived, pass: u8) {
    d.adj = IndexMap::new();
    for l in st.links.iter() {
        let info = &d.links[&l.id];
        if !info.up {
            continue;
        }
        let lossy = if info.mismatch { 0.35 } else { 0.0 };
        let ca = port_channels(st, d, &l.a.dev, &l.a.port, pass);
        let cb = port_channels(st, d, &l.b.dev, &l.b.port, pass);
        for (c, na) in ca.iter() {
            let nb = match cb.get(c) {
                Some(n) => n,
                None => continue,
            };
            if blocked_node(d, na) || blocked_node(d, nb) {
                continue;
            }
            add_edge(d, na, nb, Some(&l.id), lossy);
        }
    }
    // SVI:er
    for (id, dev) in st.devices.iter() {
        if !is_switch(dev) {
            continue;
        }
        for (n, s) in dev.cfg().ifaces.iter() {
            if let Some(v) = s.svi {
                if !s.shutdown && vlan_exists(dev, v) {
                    add_edge(d, &ek(id, n), &format!("S:{}:{}", id, v), None, 0.0);
                }
            }
        }
    }
    // Trådlösa klienter
    if pass == 2 {
        d.wifi = IndexMap::new();
        let w = st.devices.get("WLC").and_then(|x| x.wlc.as_ref());
        for (id, c) in st.devices.iter() {
            let ssid = match &c.ssid {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let w = match w {
                Some(w) if c.wireless && c.powered => w,
                _ => continue,
            };
            let wlan = match w.wlans.values().find(|wl| &wl.ssid == ssid && wl.enabled) {
                Some(x) => x,
                None => continue,
            };
            let mut aps: Vec<String> = d.ap_joined.iter().filter(|(a, j)| **j && st.devices[*a].site == c.site).map(|(a, _)| a.clone()).collect();
            aps.sort();
            if aps.is_empty() {
                continue;
            }
            let ap = aps[0].clone();
            let vlan = match w.interfaces.get(&wlan.iface) {
                Some(f) => f.vlan,
                None => continue,
            };
            let interf = channel_conflict(st, d, &c.site);
            add_edge(d, &ek(id, "nic"), &format!("W:{}:{}", ap, vlan), Some("wifi"), if interf { 0.4 } else { 0.0 });
            d.wifi.insert(id.clone(), WifiInfo { ap, vlan, ssid: ssid.clone(), interference: interf, aps });
        }
    }
    // Komponenter
    d.comp = HashMap::new();
    let mut n = 0usize;
    let starts: Vec<String> = d.adj.keys().cloned().collect();
    for start in starts {
        if d.comp.contains_key(&start) {
            continue;
        }
        let mut q: VecDeque<String> = VecDeque::new();
        d.comp.insert(start.clone(), n);
        q.push_back(start);
        while let Some(x) = q.pop_front() {
            let tos: Vec<String> = d.adj.get(&x).map(|v| v.iter().map(|e| e.to.clone()).collect()).unwrap_or_default();
            for t in tos {
                if !d.comp.contains_key(&t) {
                    d.comp.insert(t.clone(), n);
                    q.push_back(t);
                }
            }
        }
        n += 1;
    }
}

fn channel_conflict(st: &State, d: &Derived, site: &str) -> bool {
    let w = match st.devices.get("WLC").and_then(|x| x.wlc.as_ref()) {
        Some(w) => w,
        None => return false,
    };
    let mut seen: Vec<Option<u32>> = vec![];
    for (a, j) in d.ap_joined.iter() {
        if !*j || st.devices[a].site != site {
            continue;
        }
        let ch = w.ap_channels.get(a).copied();
        if seen.contains(&ch) {
            return true;
        }
        seen.push(ch);
    }
    false
}

// ---------------------------------------------------------------- Endpoints

pub fn host_ip_conf(h: &Device) -> Option<IpConf> {
    let nic = h.nic.as_ref()?;
    if nic.dhcp {
        return h.lease.as_ref().map(|l| IpConf { ip: Some(l.ip.clone()), mask: Some(l.mask.clone()), gw: l.gw.clone(), dns: l.dns.clone(), apipa: l.apipa });
    }
    nic.static_.as_ref().map(|s| IpConf { ip: Some(s.ip.clone()), mask: Some(s.mask.clone()), gw: s.gw.clone(), dns: s.dns.clone(), apipa: false })
}

fn svi_up(d: &Derived, dev: &str, v: u16) -> bool {
    let node = format!("S:{}:{}", dev, v);
    d.adj.get(&node).map(|es| es.iter().any(|e| e.link.is_some())).unwrap_or(false)
}

fn build_endpoints(st: &State, d: &mut Derived) {
    d.eps = IndexMap::new();
    for (id, dev) in st.devices.iter() {
        if dev.os == Os::Ios {
            for (n, i) in dev.cfg().ifaces.iter() {
                let ip = match &i.ip {
                    Some(ip) if !i.internal => ip,
                    _ => continue,
                };
                let k = ek(id, n);
                if i.is_loop {
                    d.eps.insert(k.clone(), Ep { key: k, dev: id.clone(), iface: n.clone(), ip: Some(ip.addr.clone()), mask: Some(ip.mask.clone()), gw: None, mac: dev.base_mac.clone(), up: !i.shutdown, is_loop: true });
                    continue;
                }
                let up = if let Some(v) = i.svi {
                    !i.shutdown && vlan_exists(dev, v) && svi_up(d, id, v)
                } else if let Some(par) = &i.parent {
                    !i.shutdown && d.ports.get(&key(id, par)).map(|p| p.up).unwrap_or(false)
                } else {
                    d.ports.get(&key(id, n)).map(|p| p.up).unwrap_or(false)
                };
                let up = up && d.comp.contains_key(&k);
                d.eps.insert(k.clone(), Ep { key: k, dev: id.clone(), iface: n.clone(), ip: Some(ip.addr.clone()), mask: Some(ip.mask.clone()), gw: None, mac: dev.base_mac.clone(), up, is_loop: false });
            }
        } else if dev.kind == Kind::Isp {
            // Operatörerna (ISP, och ISP-B i kurs 2) är förenklade enheter med en adress mot Nordvik
            let k = ek(id, "nic");
            let up = d.ports.get(&key(id, "nic")).map(|p| p.up).unwrap_or(false);
            d.eps.insert(k.clone(), Ep { key: k, dev: id.clone(), iface: "nic".into(), ip: dev.ip.clone(), mask: dev.mask.clone(), gw: None, mac: dev.base_mac.clone(), up, is_loop: false });
        } else if let Some(nic) = &dev.nic {
            let conf = host_ip_conf(dev);
            let k = ek(id, "nic");
            let mut up = dev.powered && nic.enabled && d.comp.contains_key(&k);
            if !dev.wireless {
                up = up && d.ports.get(&key(id, "nic")).map(|p| p.up).unwrap_or(false);
            }
            d.eps.insert(
                k.clone(),
                Ep { key: k, dev: id.clone(), iface: "nic".into(), ip: conf.as_ref().and_then(|c| c.ip.clone()), mask: conf.as_ref().and_then(|c| c.mask.clone()), gw: conf.as_ref().and_then(|c| c.gw.clone()), mac: nic.mac.clone(), up, is_loop: false },
            );
        }
    }
    d.by_seg = HashMap::new();
    for (k, _) in d.eps.iter() {
        if let Some(c) = d.comp.get(k) {
            d.by_seg.entry(*c).or_default().push(k.clone());
        }
    }
}

// ---------------------------------------------------------------- Port security

fn port_security(st: &mut State, d: &mut Derived) {
    d.sec_blocked = HashSet::new();
    let ids: Vec<String> = st.devices.keys().cloned().collect();
    for id in ids {
        if !is_switch(st.dev(&id)) {
            continue;
        }
        let names: Vec<String> = st.dev(&id).cfg().ifaces.keys().cloned().collect();
        for n in names {
            let (ps, shutdown, is_access) = {
                let i = &st.dev(&id).cfg().ifaces[&n];
                (i.port_sec.clone(), i.shutdown, i.mode == "access")
            };
            let ps = match ps {
                Some(p) if is_access => p,
                _ => continue,
            };
            let peer_dev = match link_at(st, &id, &n) {
                Some((l, peer)) if (l.state == LinkState::Ok || l.state == LinkState::Flapping) && !shutdown => peer.dev.clone(),
                _ => {
                    st.dev_mut(&id).rt.sec_learned.insert(n.clone(), vec![]);
                    continue;
                }
            };
            let mut macs: Vec<String> = vec![];
            let mut owners: Vec<String> = vec![];
            let peer = st.dev(&peer_dev);
            if peer.kind == Kind::Hub {
                for l in st.links.iter() {
                    let o = if l.a.dev == peer.id {
                        Some(&l.b)
                    } else if l.b.dev == peer.id {
                        Some(&l.a)
                    } else {
                        None
                    };
                    let o = match o {
                        Some(o) if o.dev != id => o,
                        _ => continue,
                    };
                    if let Some(h) = st.devices.get(&o.dev) {
                        if let Some(nic) = &h.nic {
                            if h.powered && l.state == LinkState::Ok {
                                macs.push(nic.mac.clone());
                                owners.push(h.id.clone());
                            }
                        }
                    }
                }
            } else if let Some(nic) = &peer.nic {
                if peer.powered {
                    macs.push(nic.mac.clone());
                    owners.push(peer.id.clone());
                }
            }
            let mut allowed = ps.macs.clone();
            let mut violators: Vec<String> = vec![];
            let mut sticky_add: Vec<String> = vec![];
            for (idx, m) in macs.iter().enumerate() {
                if allowed.contains(m) {
                    continue;
                }
                if (allowed.len() as u32) < ps.max {
                    allowed.push(m.clone());
                    if ps.sticky {
                        sticky_add.push(m.clone());
                    }
                } else {
                    violators.push(owners[idx].clone());
                }
            }
            {
                let sw = st.dev_mut(&id);
                if !sticky_add.is_empty() {
                    if let Some(p) = sw.cfg_mut().ifaces.get_mut(&n).and_then(|i| i.port_sec.as_mut()) {
                        p.macs.extend(sticky_add);
                    }
                }
                sw.rt.sec_learned.insert(n.clone(), macs);
            }
            if !violators.is_empty() {
                if ps.violation == "shutdown" {
                    let already = st.dev(&id).rt.errdisabled.get(&n).map(|s| !s.is_empty()).unwrap_or(false);
                    if !already {
                        let vmac = st.dev(&violators[0]).nic.as_ref().map(|x| x.mac.clone()).unwrap_or_default();
                        {
                            let sw = st.dev_mut(&id);
                            sw.rt.errdisabled.insert(n.clone(), "psecure-violation".into());
                            sw.rt.sec_violations += 1;
                        }
                        push_log(st, &id, &format!("%PORT_SECURITY-2-PSECURE_VIOLATION: Security violation occurred, caused by MAC address {} on port {}.", U::mac_dots(&vmac), n));
                        push_log(st, &id, &format!("%PM-4-ERR_DISABLE: psecure-violation error detected on {}, putting {} in err-disable state", U::short_if(&n), U::short_if(&n)));
                    }
                } else {
                    for v in violators {
                        d.sec_blocked.insert(v);
                    }
                    st.dev_mut(&id).rt.sec_violations += 1;
                }
            }
        }
    }
}

// ---------------------------------------------------------------- Beräkning

/// Räknar om allt härlett (compute i sim.js) och lägger det i state.cache.
pub fn compute(st: &mut State) -> Rc<Derived> {
    let mut d = Derived::default();
    compute_poe(st, &mut d);
    port_security(st, &mut d);
    physical(st, &mut d);
    spanning_tree(st, &mut d);
    build_l2(st, &mut d, 1);
    build_endpoints(st, &mut d);
    // OSPF och BGP (kurs 2) behövs redan här: accesspunkterna i Borås når controllern via OSPF-rutter
    dynroute::compute(st, &mut d, true);
    // Accesspunkter ansluter till controllern
    let ids: Vec<String> = st.devices.keys().cloned().collect();
    for id in ids {
        let dev = st.dev(&id);
        if dev.kind != Kind::Ap || !dev.powered {
            continue;
        }
        let o = PingOpts { proto: Some("udp".into()), dport: Some(5246), ..Default::default() };
        let r = ping_internal(st, &d, &id, "192.168.1.196", &o);
        d.ap_joined.insert(id, r.ok);
    }
    build_l2(st, &mut d, 2);
    build_endpoints(st, &mut d);
    dynroute::compute(st, &mut d, false);
    let logs = std::mem::take(&mut d.dynr.logs);
    let rc = Rc::new(d);
    st.cache = Some(rc.clone());
    st.dirty = false;
    st.ntp_cache.clear();
    for (dev, msg) in logs {
        push_log(st, &dev, &msg);
    }
    rc
}
/// Härledd data, räknas om vid behov (get i sim.js)
pub fn get(st: &mut State) -> Rc<Derived> {
    if st.cache.is_none() || st.dirty {
        return compute(st);
    }
    st.cache.clone().unwrap()
}
pub fn touch(st: &mut State) {
    st.dirty = true;
}

// ---------------------------------------------------------------- Lager 3

/// BFS-väg mellan två noder: ger förlust längs vägen
fn l2_loss(d: &Derived, from: &str, to: &str) -> f64 {
    if from == to {
        return 0.0;
    }
    let mut prev: HashMap<String, Option<String>> = HashMap::new();
    let mut edge_to: HashMap<String, f64> = HashMap::new();
    prev.insert(from.to_string(), None);
    let mut q: VecDeque<String> = VecDeque::new();
    q.push_back(from.to_string());
    while let Some(x) = q.pop_front() {
        if x == to {
            break;
        }
        if let Some(es) = d.adj.get(&x) {
            for e in es {
                if !prev.contains_key(&e.to) {
                    prev.insert(e.to.clone(), Some(x.clone()));
                    edge_to.insert(e.to.clone(), e.lossy);
                    q.push_back(e.to.clone());
                }
            }
        }
    }
    if !prev.contains_key(to) {
        return 1.0;
    }
    let mut loss: f64 = 0.0;
    let mut storm_hit = false;
    let mut storm_switch = false;
    let storm_sw: HashSet<&String> = d.storm.values().flat_map(|v| v.iter()).collect();
    let mut cur: Option<String> = Some(to.to_string());
    while let Some(c) = cur {
        if let Some(rest) = c.strip_prefix("S:") {
            if let Some((dev, vl)) = rest.rsplit_once(':') {
                if !dev.is_empty() && !dev.contains(':') && !vl.is_empty() && vl.chars().all(|ch| ch.is_ascii_digit()) {
                    if let Ok(v) = vl.parse::<u16>() {
                        if d.storm.contains_key(&v) {
                            storm_hit = true;
                        }
                    }
                    if storm_sw.contains(&dev.to_string()) {
                        storm_switch = true;
                    }
                }
            }
        }
        if c == from {
            break;
        }
        loss = loss.max(edge_to.get(&c).copied().unwrap_or(0.0));
        cur = prev.get(&c).cloned().flatten();
    }
    // Belastningen drabbar allt som går genom switcharna i cirkeln
    if storm_switch {
        loss = loss.max(0.7);
    }
    if storm_hit {
        loss = 1.0;
    }
    loss
}

/// Vem svarar på ARP för ip i segmentet
fn arp_owner(st: &State, d: &Derived, seg: usize, ip: &str, except_key: &str) -> Option<Ep> {
    let list = d.seg_eps(seg);
    if let Some(h) = list.iter().find(|e| e.up && e.ip.as_deref() == Some(ip) && e.key != except_key) {
        return Some((*h).clone());
    }
    // Proxy-ARP för statisk NAT på utsidan
    for e in list {
        if !e.up {
            continue;
        }
        let dev = st.dev(&e.dev);
        if is_router(dev) {
            let ic = match dev.ifc(&e.iface) {
                Some(i) => i,
                None => continue,
            };
            if ic.nat_dir == Some(NatDir::Outside)
                && dev.cfg().nat.statics.iter().any(|s| s.global == ip && U::same_subnet(ip, e.ip.as_deref().unwrap_or(""), e.mask.as_deref().unwrap_or("0.0.0.0")))
            {
                return Some(e.clone());
            }
        }
    }
    None
}

fn router_owns(d: &Derived, dev: &str, ip: &str) -> bool {
    dev_eps(d, dev).iter().any(|e| e.up && e.ip.as_deref() == Some(ip))
}

struct Cand {
    kind: String,
    net: String,
    mask: String,
    nh: Option<String>,
    iface: String,
    ep_key: String,
    ad: i64,
    metric: i64,
    direct: bool,
}

fn ad_of(c: &Cand) -> i64 {
    if c.kind == "C" { 0 } else if c.ad == 0 { 1 } else { c.ad }
}

pub fn route_lookup(st: &State, d: &Derived, dev: &str, dst: &str) -> Option<RouteHit> {
    let dd = st.dev(dev);
    let mut cand: Vec<Cand> = vec![];
    let eps = dev_eps(d, dev);
    for e in eps.iter() {
        if !e.up {
            continue;
        }
        let ip = e.ip.clone().unwrap_or_default();
        let mask = e.mask.clone().unwrap_or_default();
        cand.push(Cand { kind: "C".into(), net: U::network(&ip, &mask), mask, nh: None, iface: e.iface.clone(), ep_key: e.key.clone(), ad: 0, metric: 0, direct: false });
    }
    for r in dd.cfg().routes.iter() {
        let rad = r.ad.filter(|a| *a != 0).unwrap_or(1);
        if let Some(nh) = &r.nh {
            // Rekursiv upplösning: nästa hopp måste ligga i ett anslutet nät
            let mut via: Option<&&Ep> = None;
            for e in eps.iter() {
                if e.up && e.ip.is_some() && U::same_subnet(nh, e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("0.0.0.0")) {
                    via = Some(e);
                }
            }
            if let Some(v) = via {
                cand.push(Cand { kind: "S".into(), net: r.net.clone(), mask: r.mask.clone(), nh: Some(nh.clone()), iface: v.iface.clone(), ep_key: v.key.clone(), ad: rad, metric: 0, direct: false });
            }
        } else if let Some(i) = &r.iface {
            if let Some(ep) = d.ep(dev, i) {
                if ep.up {
                    cand.push(Cand { kind: "S".into(), net: r.net.clone(), mask: r.mask.clone(), nh: None, iface: i.clone(), ep_key: ep.key.clone(), ad: rad, metric: 0, direct: true });
                }
            }
        }
    }
    // OSPF- och BGP-rutter (kurs 2)
    if let Some(rs) = d.dynr.routes.get(dev) {
        for r in rs {
            if d.eps.get(&r.ep_key).map(|e| e.up).unwrap_or(false) {
                cand.push(Cand { kind: r.kind.clone(), net: r.net.clone(), mask: r.mask.clone(), nh: Some(r.nh.clone()), iface: r.iface.clone(), ep_key: r.ep_key.clone(), ad: r.ad, metric: r.metric, direct: false });
            }
        }
    }
    let mut best: Option<(usize, u32)> = None;
    for (idx, c) in cand.iter().enumerate() {
        if U::in_net(dst, &c.net, &c.mask) {
            let p = U::mask_to_prefix(&c.mask).unwrap_or(0);
            // Längsta prefix vinner; vid lika prefix vinner lägst administrativt avstånd
            let take = match best {
                None => true,
                Some((bi, bp)) => p > bp || (p == bp && ad_of(c) < ad_of(&cand[bi])),
            };
            if take {
                best = Some((idx, p));
            }
        }
    }
    best.map(|(i, p)| {
        let c = &cand[i];
        RouteHit { kind: c.kind.clone(), net: c.net.clone(), mask: c.mask.clone(), nh: c.nh.clone(), iface: c.iface.clone(), ep_key: c.ep_key.clone(), ad: ad_of(c), metric: c.metric, direct: c.direct, prefix: p }
    })
}

pub fn installed_routes(st: &State, d: &Derived, dev: &str) -> Vec<InstalledRoute> {
    let dd = st.dev(dev);
    let mut out: Vec<InstalledRoute> = vec![];
    let eps = dev_eps(d, dev);
    for e in eps.iter() {
        if !e.up {
            continue;
        }
        let ip = e.ip.clone().unwrap_or_default();
        let mask = e.mask.clone().unwrap_or_default();
        out.push(InstalledRoute { kind: "C".into(), net: U::network(&ip, &mask), mask: mask.clone(), nh: None, iface: Some(e.iface.clone()), ad: 0, metric: 0, path: vec![] });
        if mask != "255.255.255.255" {
            out.push(InstalledRoute { kind: "L".into(), net: ip, mask: "255.255.255.255".into(), nh: None, iface: Some(e.iface.clone()), ad: 0, metric: 0, path: vec![] });
        }
    }
    let mut stl: Vec<InstalledRoute> = vec![];
    for r in dd.cfg().routes.iter() {
        let dflt = r.net == "0.0.0.0" && r.mask == "0.0.0.0";
        let kind = if dflt { "S*" } else { "S" };
        let rad = r.ad.filter(|a| *a != 0).unwrap_or(1);
        if let Some(nh) = &r.nh {
            let ok = eps.iter().any(|e| e.up && e.ip.is_some() && U::same_subnet(nh, e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("0.0.0.0")));
            if ok {
                stl.push(InstalledRoute { kind: kind.into(), net: r.net.clone(), mask: r.mask.clone(), nh: Some(nh.clone()), iface: None, ad: rad, metric: 0, path: vec![] });
            }
        } else if let Some(i) = &r.iface {
            if d.ep(dev, i).map(|e| e.up).unwrap_or(false) {
                stl.push(InstalledRoute { kind: kind.into(), net: r.net.clone(), mask: r.mask.clone(), nh: None, iface: Some(i.clone()), ad: rad, metric: 0, path: vec![] });
            }
        }
    }
    if let Some(rs) = d.dynr.routes.get(dev) {
        for r in rs {
            if d.eps.get(&r.ep_key).map(|e| e.up).unwrap_or(false) {
                stl.push(InstalledRoute { kind: r.kind.clone(), net: r.net.clone(), mask: r.mask.clone(), nh: Some(r.nh.clone()), iface: Some(r.iface.clone()), ad: r.ad, metric: r.metric, path: r.path.clone() });
            }
        }
    }
    // Bara rutterna med lägst administrativt avstånd per nät installeras
    for r in stl.iter() {
        let best = stl.iter().all(|q| q.net != r.net || q.mask != r.mask || q.ad >= r.ad);
        if best {
            out.push(r.clone());
        }
    }
    out
}

// ACL-utvärdering
fn addr_match(spec: Option<&AddrSpec>, ip: &str) -> bool {
    let spec = match spec {
        None => return true,
        Some(s) if s.any => return true,
        Some(s) => s,
    };
    if let Some(h) = &spec.host {
        return h == ip;
    }
    U::wild_match(ip, spec.ip.as_deref().unwrap_or("0.0.0.0"), spec.wild.as_deref().unwrap_or("0.0.0.0"))
}

fn acl_match(extended: bool, r: &Ace, pkt: &AclPkt) -> bool {
    if !addr_match(r.src.as_ref(), &pkt.src) {
        return false;
    }
    if !extended {
        return true;
    }
    if let Some(p) = &r.proto {
        if p != "ip" && *p != pkt.proto {
            return false;
        }
    }
    if !addr_match(r.dst.as_ref(), &pkt.dst) {
        return false;
    }
    if let Some(rd) = r.dport.filter(|x| *x != 0) {
        let pd = match pkt.dport {
            Some(p) => p,
            None => return false,
        };
        let op = r.dop.as_deref().unwrap_or("eq");
        if op == "eq" && pd != rd {
            return false;
        }
        if op == "neq" && pd == rd {
            return false;
        }
        if op == "gt" && !(pd > rd) {
            return false;
        }
        if op == "lt" && !(pd < rd) {
            return false;
        }
        if op == "range" && !(pd >= rd && pd <= r.dport2.unwrap_or(0)) {
            return false;
        }
    }
    true
}

fn pkt_acl(p: &Pkt) -> AclPkt {
    AclPkt { src: p.src.clone().unwrap_or_default(), dst: p.dst.clone(), proto: p.proto.clone(), dport: p.dport }
}

fn acl_check(st: &mut State, dev: &str, name: Option<&str>, pkt: &Pkt) -> bool {
    let name = match name {
        Some(n) if !n.is_empty() => n,
        _ => return true,
    };
    let ap = pkt_acl(pkt);
    let d = st.dev_mut(dev);
    let acl = match d.cfg().acls.get(name) {
        Some(a) => a.clone(),
        None => return true, // Ciscos beteende: en lista som inte finns släpper allt
    };
    for r in acl.rules.iter() {
        if r.remark.is_some() {
            continue;
        }
        if acl_match(acl.extended, r, &ap) {
            let c = d.rt.acl_counters.entry(name.to_string()).or_default();
            *c.entry(r.seq).or_insert(0.0) += pkt.count.unwrap_or(1.0);
            return r.action == "permit";
        }
    }
    false
}

/// Utvärdera en ACL utan att räkna träffar (för kontroller)
pub fn acl_eval(st: &State, dev: &str, name: &str, pkt: &AclPkt) -> bool {
    let acl = match st.dev(dev).cfg().acls.get(name) {
        Some(a) => a,
        None => return true,
    };
    for r in acl.rules.iter() {
        if r.remark.is_some() {
            continue;
        }
        if acl_match(acl.extended, r, pkt) {
            return r.action == "permit";
        }
    }
    false
}

fn nat_match_list(st: &State, dev: &str, acl_name: &str, src: &str, dst: &str) -> bool {
    let acl = match st.dev(dev).cfg().acls.get(acl_name) {
        Some(a) => a,
        None => return false,
    };
    let p = AclPkt { src: src.into(), dst: if dst.is_empty() { "0.0.0.0".into() } else { dst.into() }, proto: "ip".into(), dport: None };
    for r in acl.rules.iter() {
        if r.remark.is_some() {
            continue;
        }
        if acl_match(acl.extended, r, &p) {
            return r.action == "permit";
        }
    }
    false
}

// ---------------------------------------------------------------- IPsec (kapitel 10)
// Crypto map-tunnlar mellan två routrar över internet. NAT sker före kryptering,
// precis som i IOS, så en NAT-lista utan deny-rad gör att trafiken aldrig krypteras.

#[derive(Clone, Debug)]
struct CEntry {
    seq: i64,
    map: String,
    peer: Option<String>,
    ts: Option<String>,
    acl: Option<String>,
}

fn crypto_entries(d: &Device, map_name: &str) -> Vec<CEntry> {
    let cr = match d.cfg().crypto.as_ref() {
        Some(c) => c,
        None => return vec![],
    };
    match cr.maps.get(map_name) {
        Some(m) => m.iter().map(|(seq, e)| CEntry { seq: *seq, map: map_name.to_string(), peer: e.peer.clone(), ts: e.ts.clone(), acl: e.acl.clone() }).collect(),
        None => vec![],
    }
}

struct CMatch {
    entry: CEntry,
    rule: Ace,
    iface: String,
}

/// Vilken crypto map-rad (och ACL-rad) matchar paketet på väg ut genom ifname?
fn crypto_match(st: &State, dev: &str, ifname: &str, pkt: &Pkt) -> Option<CMatch> {
    let d = st.dev(dev);
    let i = d.ifc(ifname)?;
    let cm = i.crypto_map.as_ref()?;
    let ap = AclPkt { src: pkt.src.clone().unwrap_or_default(), dst: pkt.dst.clone(), proto: pkt.proto.clone(), dport: None };
    for e in crypto_entries(d, cm) {
        let (_, _, acl_name) = match (&e.peer, &e.ts, &e.acl) {
            (Some(p), Some(t), Some(a)) => (p, t, a),
            _ => continue, // ofullständig rad används inte
        };
        let acl = match d.cfg().acls.get(acl_name) {
            Some(a) if a.extended => a,
            _ => continue,
        };
        for r in acl.rules.iter() {
            if r.remark.is_some() {
                continue;
            }
            if acl_match(true, r, &ap) {
                if r.action == "deny" {
                    break;
                }
                return Some(CMatch { entry: e.clone(), rule: r.clone(), iface: ifname.to_string() });
            }
        }
    }
    None
}

fn router_by_ip(st: &State, d: &Derived, ip: &str) -> Option<Ep> {
    d.eps.values().find(|e| e.up && e.ip.as_deref() == Some(ip) && is_router(st.dev(&e.dev))).cloned()
}

struct P1 {
    ok: bool,
    reason: &'static str,
    state: &'static str,
    peer_dev: String,
    peer_ep: Option<Ep>,
    peer_entries: Vec<CEntry>,
    local_ip: String,
}

fn p1_fail(reason: &'static str, state: &'static str) -> P1 {
    P1 { ok: false, reason, state, peer_dev: String::new(), peer_ep: None, peer_entries: vec![], local_ip: String::new() }
}

/// Fas 1 (ISAKMP) för en crypto map-rad. Returnerar peer-info eller orsak.
fn phase1(st: &State, d: &Derived, dev: &str, e: &CEntry, ifname: &str) -> P1 {
    let dd = st.dev(dev);
    let cr = match dd.cfg().crypto.as_ref() {
        Some(c) => c,
        None => return p1_fail("nomap", "MM_NO_STATE"),
    };
    let peer = e.peer.clone().unwrap_or_default();
    let local = match d.eps.get(&ek(dev, ifname)) {
        Some(l) if l.up => l,
        _ => return p1_fail("down", "MM_NO_STATE"),
    };
    let pe = match router_by_ip(st, d, &peer) {
        Some(p) => p,
        None => return p1_fail("nopeer", "MM_NO_STATE"),
    };
    // Nås peer över internet?
    if route_lookup(st, d, dev, &peer).is_none() {
        return p1_fail("noroute", "MM_NO_STATE");
    }
    let pd = st.dev(&pe.dev);
    let pcr = pd.cfg().crypto.as_ref();
    let pcm = pd.ifc(&pe.iface).and_then(|i| i.crypto_map.clone());
    let (pcr, pcm) = match (pcr, pcm) {
        (Some(a), Some(b)) => (a, b),
        _ => return p1_fail("peer-nomap", "MM_NO_STATE"),
    };
    let local_ip = local.ip.clone().unwrap_or_default();
    let p_entries: Vec<CEntry> = crypto_entries(pd, &pcm).into_iter().filter(|x| x.peer.as_deref() == Some(local_ip.as_str())).collect();
    if p_entries.is_empty() {
        return p1_fail("peer-nomap", "MM_NO_STATE");
    }
    let mtch = cr.policies.values().any(|a| pcr.policies.values().any(|b| a.enc == b.enc && a.hash == b.hash && a.auth == b.auth && a.group == b.group));
    if !mtch {
        return p1_fail("policy", "MM_NO_STATE");
    }
    let k1 = cr.keys.get(&peer);
    let k2 = pcr.keys.get(&local_ip);
    match (k1, k2) {
        (Some(a), Some(b)) if a == b => {}
        _ => return p1_fail("key", "MM_KEY_EXCH"),
    }
    P1 { ok: true, reason: "", state: "QM_IDLE", peer_dev: pe.dev.clone(), peer_ep: Some(pe), peer_entries: p_entries, local_ip }
}

/// Fas 2 (IPsec SA) för en ACL-rad: transform-set och spegelvänd ACL på andra sidan
fn phase2(st: &State, dev: &str, e: &CEntry, rule: &Ace, p1: &P1) -> Result<(CEntry, Ace), &'static str> {
    let d = st.dev(dev);
    let cr = d.cfg().crypto.as_ref();
    let pd = st.dev(&p1.peer_dev);
    let pcr = pd.cfg().crypto.as_ref();
    let ts = cr.and_then(|c| e.ts.as_ref().and_then(|t| c.transform_sets.get(t)));
    for pe in p1.peer_entries.iter() {
        let pts = pcr.and_then(|c| pe.ts.as_ref().and_then(|t| c.transform_sets.get(t)));
        let (ts, pts) = match (ts, pts) {
            (Some(a), Some(b)) if a.transforms.join(" ") == b.transforms.join(" ") => (a, b),
            _ => continue,
        };
        let _ = (ts, pts);
        let pacl = match pe.acl.as_ref().and_then(|a| pd.cfg().acls.get(a)) {
            Some(a) => a,
            None => continue,
        };
        for r in pacl.rules.iter() {
            if mirrors(rule, r) {
                return Ok((pe.clone(), r.clone()));
            }
        }
    }
    Err(if ts.is_some() { "proxy" } else { "transform" })
}

fn sa_key(e: &CEntry, rule: &Ace) -> String {
    format!("{}:{}:{}", e.map, e.seq, rule.seq)
}

fn ike_mark(st: &mut State, dev: &str, peer_ip: &str, up: bool) {
    let time = st.time;
    let d = st.dev_mut(dev);
    match d.rt.ike.get_mut(peer_ip) {
        Some(cur) => cur.up = up,
        None => {
            d.rt.ike.insert(peer_ip.to_string(), Ike { since: time, conn_id: 1001 + (U::hash(&format!("{}{}", dev, peer_ip)) % 5), up });
        }
    }
}

/// Försöker skicka paketet genom tunneln. Räknar encaps/decaps och loggar fel som IOS gör.
fn tunnel_for(st: &mut State, d: &Derived, dev: &str, cm: &CMatch, pkt: &Pkt) -> Result<(String, Ep), String> {
    let e = &cm.entry;
    let peer = e.peer.clone().unwrap_or_default();
    let p1 = phase1(st, d, dev, e, &cm.iface);
    ike_mark(st, dev, &peer, p1.ok);
    if !p1.ok {
        return Err(p1.reason.to_string());
    }
    ike_mark(st, &p1.peer_dev, &p1.local_ip, true);
    let p2 = phase2(st, dev, e, &cm.rule, &p1);
    let pdn = p1.peer_dev.clone();
    let (pe_entry, pe_rule) = match p2 {
        Ok(x) => x,
        Err(reason) => {
            let fk = sa_key(e, &cm.rule);
            let seen = st.dev(&pdn).rt.p2fail.get(&fk).copied().unwrap_or(false);
            if !seen {
                st.dev_mut(&pdn).rt.p2fail.insert(fk, true);
                push_log(st, &pdn, &format!("%CRYPTO-6-IKMP_MODE_FAILURE: Processing of Quick mode failed with peer at {}", p1.local_ip));
                if reason == "proxy" {
                    push_log(st, &pdn, "%CRYPTO-4-IKMP_NO_SA: IPSEC(validate_transform_proposal): proxy identities not supported");
                }
                push_log(st, dev, &format!("%CRYPTO-6-IKMP_MODE_FAILURE: Processing of Quick mode failed with peer at {}", peer));
            }
            return Err(reason.to_string());
        }
    };
    let n = pkt.count.unwrap_or(1.0);
    let time = st.time;
    let k = sa_key(e, &cm.rule);
    let pk = sa_key(&pe_entry, &pe_rule);
    st.dev_mut(dev).rt.ipsec.entry(k).or_insert(SaCount { encaps: 0.0, decaps: 0.0, since: time }).encaps += n;
    let pd = st.dev_mut(&pdn);
    pd.rt.ipsec.entry(pk).or_insert(SaCount { encaps: 0.0, decaps: 0.0, since: time }).decaps += n;
    pd.rt.p2fail.clear();
    Ok((pdn, p1.peer_ep.unwrap()))
}

/// Status för show crypto: fas 1 per peer och SA per ACL-rad
pub fn crypto_status(st: &mut State, dev: &str) -> CryptoStatus {
    let d = get(st);
    let dd = st.dev(dev);
    let mut out = CryptoStatus::default();
    if dd.cfg().crypto.is_none() {
        return out;
    }
    for (n, i) in dd.cfg().ifaces.iter() {
        let cm = match &i.crypto_map {
            Some(c) => c,
            None => continue,
        };
        let local = d.eps.get(&ek(dev, n));
        for e in crypto_entries(dd, cm) {
            let peer = match &e.peer {
                Some(p) => p.clone(),
                None => continue,
            };
            let ike = dd.rt.ike.get(&peer);
            let p1 = phase1(st, &d, dev, &e, n);
            if let Some(ike) = ike {
                out.isakmp.push(IsakmpStatus {
                    dst: peer.clone(),
                    src: local.and_then(|l| l.ip.clone()).unwrap_or_else(|| i.ip.as_ref().map(|x| x.addr.clone()).unwrap_or_default()),
                    state: if p1.ok { "QM_IDLE".into() } else { p1.state.to_string() },
                    conn_id: if p1.ok { ike.conn_id } else { 0 },
                    status: "ACTIVE".into(),
                });
            }
            let acl = match e.acl.as_ref().and_then(|a| dd.cfg().acls.get(a)) {
                Some(a) => a,
                None => continue,
            };
            for r in acl.rules.iter() {
                if r.remark.is_some() || r.action != "permit" {
                    continue;
                }
                let up = p1.ok && ike.is_some() && phase2(st, dev, &e, r, &p1).is_ok();
                let (enc, dec) = dd.rt.ipsec.get(&sa_key(&e, r)).map(|c| (c.encaps, c.decaps)).unwrap_or((0.0, 0.0));
                out.sas.push(SaStatus {
                    iface: n.clone(),
                    map: e.map.clone(),
                    seq: e.seq,
                    local: local.and_then(|l| l.ip.clone()).unwrap_or_default(),
                    peer: peer.clone(),
                    rule: r.clone(),
                    up,
                    encaps: enc,
                    decaps: dec,
                    spi: U::hash(&format!("{}{}{}", dev, peer, r.seq)),
                    mtu: TUNNEL_MTU,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------- Skicka paket

fn ok_res(at: &str, pkt: Pkt, hops: Vec<Hop>, loss: f64) -> SendResult {
    SendResult { ok: true, at: Some(at.to_string()), pkt, hops, loss, ..Default::default() }
}

fn isp_for(st: &State, ip: &str, prefer: Option<&str>) -> Option<String> {
    let ids: Vec<&String> = st
        .devices
        .iter()
        .filter(|(_, d)| d.kind == Kind::Isp && d.ip.is_some() && U::same_subnet(ip, d.ip.as_deref().unwrap(), d.mask.as_deref().unwrap_or("0.0.0.0")))
        .map(|(id, _)| id)
        .collect();
    if let Some(p) = prefer {
        if ids.iter().any(|i| i.as_str() == p) {
            return Some(p.to_string());
        }
    }
    ids.first().map(|s| s.to_string())
}

/// Skickar ett paket från en enhet. Returnerar var det tog vägen.
fn send(st: &mut State, d: &Derived, from: &str, pkt0: Pkt, opts: &PingOpts) -> SendResult {
    let mut hops: Vec<Hop> = vec![];
    let mut loss: f64 = 0.0;
    let mut dev = from.to_string();
    let mut ingress: Option<Ep> = None;
    let mut pkt = Pkt { proto: if pkt0.proto.is_empty() { "icmp".into() } else { pkt0.proto.clone() }, ttl: if pkt0.ttl == 0 { 64 } else { pkt0.ttl }, ..pkt0 };
    macro_rules! fail {
        ($reason:expr, $w:expr) => {
            SendResult { ok: false, reason: Some($reason.to_string()), where_: $w, hops: hops.clone(), loss, ..Default::default() }
        };
    }
    // En router eller switch som pingar en av sina egna adresser (t.ex. en loopback) svarar direkt
    if let Some(fd) = st.devices.get(from) {
        if fd.os == Os::Ios && router_owns(d, from, &pkt.dst) {
            if pkt.src.is_none() {
                pkt.src = Some(pkt.dst.clone());
            }
            return ok_res(from, pkt, vec![], 0.0);
        }
    }
    for _step in 0..40 {
        let kind = st.dev(&dev).kind;
        let egress: Ep;
        let nh: String;
        if kind == Kind::Isp {
            let isp_ip = st.dev(&dev).ip.clone().unwrap_or_default();
            if ingress.is_some() && pkt.dst == isp_ip {
                return ok_res(&dev, pkt, hops, loss);
            }
            if internet_host(&pkt.dst).is_some() {
                hops.push(Hop { dev: dev.clone(), ip: isp_ip });
                let at = format!("INTERNET:{}", pkt.dst);
                return ok_res(&at, pkt, hops, loss);
            }
            if U::is_private(&pkt.dst) {
                return fail!("drop", Some(dev.clone()));
            }
            if ingress.is_some() {
                hops.push(Hop { dev: dev.clone(), ip: isp_ip });
            }
            // Operatörens eget nät mot kunden, eller den andra operatörens (de är sammankopplade)
            let eg = isp_for(st, &pkt.dst, Some(&dev)).and_then(|o| d.eps.get(&ek(&o, "nic")).cloned());
            match eg {
                Some(e) => {
                    egress = e;
                    nh = pkt.dst.clone();
                }
                None => return fail!("drop", Some(dev.clone())),
            }
        } else if kind == Kind::Router {
            let ing_ip = ingress.as_ref().and_then(|i| i.ip.clone());
            if let Some(ing) = &ingress {
                let (acl_in, nat_dir) = {
                    let i = st.dev(&dev).ifc(&ing.iface);
                    (i.and_then(|i| i.acl_in.clone()), i.and_then(|i| i.nat_dir))
                };
                if !acl_check(st, &dev, acl_in.as_deref(), &pkt) {
                    return SendResult { iface: Some(ing.iface.clone()), from_ip: ing.ip.clone(), ..fail!("acl", Some(dev.clone())) };
                }
                if nat_dir == Some(NatDir::Outside) {
                    if let Some(tr) = nat_lookup_global(st, &dev, &pkt.dst, pkt.nat_local.as_deref()) {
                        pkt.dst = tr;
                    }
                }
                hops.push(Hop { dev: dev.clone(), ip: ing.ip.clone().unwrap_or_default() });
            }
            if router_owns(d, &dev, &pkt.dst) {
                return ok_res(&dev, pkt, hops, loss);
            }
            let rt = match route_lookup(st, d, &dev, &pkt.dst) {
                Some(r) => r,
                None => return SendResult { from_ip: ing_ip, ..fail!("net-unreachable", Some(dev.clone())) },
            };
            pkt.ttl -= 1;
            if pkt.ttl <= 0 {
                return SendResult { from_ip: ing_ip, ..fail!("ttl", Some(dev.clone())) };
            }
            let (acl_out, out_nat, out_mss, out_crypto) = {
                let o = st.dev(&dev).ifc(&rt.iface);
                (o.and_then(|o| o.acl_out.clone()), o.and_then(|o| o.nat_dir), o.and_then(|o| o.adjust_mss), o.map(|o| o.crypto_map.is_some()).unwrap_or(false))
            };
            let rt_ep = d.eps[&rt.ep_key].clone();
            if ingress.is_none() && pkt.src.is_none() {
                pkt.src = rt_ep.ip.clone();
            }
            if !acl_check(st, &dev, acl_out.as_deref(), &pkt) {
                return SendResult { iface: Some(rt.iface.clone()), from_ip: ing_ip, ..fail!("acl", Some(dev.clone())) };
            }
            let (in_nat, in_mss) = match &ingress {
                Some(ing) => {
                    let i = st.dev(&dev).ifc(&ing.iface);
                    (i.and_then(|i| i.nat_dir), i.and_then(|i| i.adjust_mss))
                }
                None => (None, None),
            };
            if ingress.is_some() && in_nat == Some(NatDir::Inside) && out_nat == Some(NatDir::Outside) {
                let local0 = pkt.src.clone();
                if let Some(g) = nat_translate(st, &dev, &pkt, &rt.iface, d) {
                    pkt.src = Some(g);
                    pkt.nat_local = local0;
                }
            }
            // ip tcp adjust-mss på vägen: SYN-paketen skrivs om i båda riktningarna
            let mss_if = in_mss.filter(|m| *m != 0).or(out_mss.filter(|m| *m != 0));
            if let Some(m) = mss_if {
                pkt.mss = Some(pkt.mss.unwrap_or(65535).min(m));
            }
            if out_crypto {
                if let Some(cm) = crypto_match(st, &dev, &rt.iface, &pkt) {
                    match tunnel_for(st, d, &dev, &cm, &pkt) {
                        Err(reason) => {
                            return SendResult { ipsec: Some(reason), from_ip: ing_ip, ..fail!("ipsec", Some(dev.clone())) };
                        }
                        Ok((pdev, pep)) => {
                            if let (Some(sz), true) = (opts.size, opts.df) {
                                if sz != 0 && sz > TUNNEL_MTU {
                                    return SendResult { mtu: Some(TUNNEL_MTU), from_ip: ing_ip.or(rt_ep.ip.clone()), ..fail!("frag", Some(dev.clone())) };
                                }
                            }
                            pkt.tunnel = true;
                            dev = pdev;
                            ingress = Some(pep);
                            continue;
                        }
                    }
                }
            }
            nh = rt.nh.clone().unwrap_or_else(|| pkt.dst.clone());
            egress = rt_ep;
        } else {
            // Värd (eller switch som skickar från sitt drift-interface)
            let mut me: Option<Ep> = d.eps.get(&ek(&dev, "nic")).cloned();
            if kind == Kind::Switch {
                let gw = st.dev(&dev).cfg().default_gateway.clone();
                me = dev_eps(d, &dev).into_iter().find(|e| e.up).map(|e| Ep { gw: gw.clone(), ..e.clone() });
                if ingress.is_some() {
                    if dev_eps(d, &dev).iter().any(|e| e.up && e.ip.as_deref() == Some(pkt.dst.as_str())) {
                        return ok_res(&dev, pkt, hops, loss);
                    }
                    return fail!("drop", Some(dev.clone()));
                }
            }
            if ingress.is_some() {
                if me.as_ref().and_then(|m| m.ip.as_deref()) == Some(pkt.dst.as_str()) {
                    return ok_res(&dev, pkt, hops, loss);
                }
                return fail!("drop", Some(dev.clone()));
            }
            let me = match me {
                Some(m) if m.up => m,
                _ => return fail!("nolink", Some(dev.clone())),
            };
            let my_ip = match &me.ip {
                Some(ip) if !ip.is_empty() => ip.clone(),
                _ => return fail!("noip", Some(dev.clone())),
            };
            if pkt.dst == my_ip {
                return ok_res(&dev, pkt, hops, 0.0);
            }
            if pkt.src.is_none() {
                pkt.src = Some(my_ip.clone());
            }
            if U::same_subnet(&pkt.dst, &my_ip, me.mask.as_deref().unwrap_or("0.0.0.0")) {
                nh = pkt.dst.clone();
            } else if let Some(gw) = me.gw.clone().filter(|g| !g.is_empty()) {
                nh = gw;
            } else {
                return fail!("nogw", Some(dev.clone()));
            }
            egress = me;
        }
        // ARP i utgående segment
        let seg = match d.comp.get(&egress.key) {
            Some(s) => *s,
            None => return SendResult { nh: Some(nh), from_ip: egress.ip.clone(), ..fail!("arp", Some(dev.clone())) },
        };
        let owner = match arp_owner(st, d, seg, &nh, &egress.key) {
            Some(o) => o,
            None => return SendResult { nh: Some(nh), from_ip: egress.ip.clone(), local: dev == from, ..fail!("arp", Some(dev.clone())) },
        };
        if opts.learn_arp {
            learn_arp(st, &dev, &nh, &owner);
        }
        loss = 1.0 - (1.0 - loss) * (1.0 - l2_loss(d, &egress.key, &owner.key));
        if st.dev(&owner.dev).kind == Kind::Hub {
            return fail!("drop", None);
        }
        dev = owner.dev.clone();
        ingress = Some(owner);
    }
    fail!("ttl", Some(dev.clone()))
}

fn learn_arp(st: &mut State, dev: &str, ip: &str, owner: &Ep) {
    let time = st.time;
    let d = st.dev_mut(dev);
    if d.nic.is_some() {
        d.arp.insert(ip.to_string(), ArpEntry { mac: owner.mac.clone(), t: time });
    }
}

fn nat_lookup_global(st: &State, dev: &str, dst: &str, nat_local: Option<&str>) -> Option<String> {
    let d = st.dev(dev);
    if let Some(s) = d.cfg().nat.statics.iter().find(|s| s.global == dst) {
        return Some(s.local.clone());
    }
    d.rt.nat_trans.iter().find(|t| t.global == dst && nat_local.map(|n| t.local == n).unwrap_or(true)).map(|t| t.local.clone())
}

/// NAT-undantag: en deny-rad i NAT-listan som träffar (källa, mål) stoppar all översättning,
/// även den statiska (i IOS görs det med route-map, här räcker listan)
fn nat_exempt(st: &State, dev: &str, pkt: &Pkt) -> bool {
    let d = st.dev(dev);
    let ap = AclPkt { src: pkt.src.clone().unwrap_or_default(), dst: pkt.dst.clone(), proto: "ip".into(), dport: None };
    d.cfg().nat.dynamic.iter().any(|r| {
        let acl = match d.cfg().acls.get(&r.acl) {
            Some(a) if a.extended => a,
            _ => return false,
        };
        for x in acl.rules.iter() {
            if x.remark.is_some() {
                continue;
            }
            if acl_match(true, x, &ap) {
                return x.action == "deny";
            }
        }
        false
    })
}

fn nat_translate(st: &mut State, dev: &str, pkt: &Pkt, egress_if: &str, d: &Derived) -> Option<String> {
    if nat_exempt(st, dev, pkt) {
        return None;
    }
    let src = pkt.src.clone().unwrap_or_default();
    let cfg_nat = st.dev(dev).cfg().nat.clone();
    if let Some(s) = cfg_nat.statics.iter().find(|s| s.local == src) {
        add_trans(st.dev_mut(dev), pkt, &s.global, &s.local, false);
        return Some(s.global.clone());
    }
    for r in cfg_nat.dynamic.iter() {
        if r.iface != egress_if {
            continue;
        }
        if !nat_match_list(st, dev, &r.acl, &src, &pkt.dst) {
            continue;
        }
        let ep_ip = match d.ep(dev, &r.iface).and_then(|e| e.ip.clone()) {
            Some(ip) => ip,
            None => continue,
        };
        if !r.overload && st.dev(dev).rt.nat_trans.iter().any(|t| t.global == ep_ip && t.local != src) {
            continue;
        }
        add_trans(st.dev_mut(dev), pkt, &ep_ip, &src, true);
        return Some(ep_ip);
    }
    None
}

fn add_trans(d: &mut Device, pkt: &Pkt, global: &str, local: &str, dynamic: bool) {
    let port = if pkt.proto == "icmp" { 1 + (U::hash(local) % 50) as i64 } else { 49152 + (U::hash(&format!("{}{}", local, pkt.dst)) % 16000) as i64 };
    let exists = d.rt.nat_trans.iter().any(|t| t.local == local && t.dst == pkt.dst && t.proto == pkt.proto);
    if !exists && dynamic {
        d.rt.nat_trans.push(NatTrans { proto: pkt.proto.clone(), global: global.into(), local: local.into(), port, dst: pkt.dst.clone(), dport: pkt.dport.filter(|p| *p != 0).unwrap_or(port), from_static: false });
    }
    if !dynamic && !exists && pkt.proto != "icmp" {
        let dp = pkt.dport.unwrap_or(0);
        d.rt.nat_trans.push(NatTrans { proto: pkt.proto.clone(), global: global.into(), local: local.into(), port: dp, dst: pkt.dst.clone(), dport: dp, from_static: true });
    }
}

fn listens(st: &State, at: &str, proto: &str, dport: Option<i64>) -> bool {
    if proto == "icmp" {
        return true;
    }
    if let Some(ip) = at.strip_prefix("INTERNET:") {
        return match internet_host(ip) {
            Some((_, dns, web)) => (dport == Some(53) && dns) || ((dport == Some(80) || dport == Some(443)) && web),
            None => false,
        };
    }
    let d = match st.devices.get(at) {
        Some(d) => d,
        None => return false,
    };
    let p = match dport {
        Some(p) => p,
        None => return d.kind == Kind::Printer && false,
    };
    if d.os == Os::Ios {
        if p == 22 {
            return ssh_enabled(d).is_some() && vty_allows(d, "ssh");
        }
        if p == 23 {
            return vty_allows(d, "telnet");
        }
        if p == 53 {
            return d.cfg().dns_server;
        }
        return p == 67;
    }
    if d.kind == Kind::Wlc {
        return p == 5246 || p == 443 || p == 22;
    }
    if d.kind == Kind::Lb {
        return (p == 80 && lb_pool(d, None).is_some()) || p == 22 || p == 443;
    }
    let has = |s: &str| d.services.iter().any(|x| x == s);
    if (p == 80 || p == 8080) && has("http") {
        return true;
    }
    if p == 445 && has("smb") {
        return true;
    }
    if p == 123 && has("ntp") {
        return true;
    }
    if p == 514 && has("syslog") {
        return true;
    }
    if p == 443 && has("https") {
        return true;
    }
    if p == 631 && d.kind == Kind::Printer {
        return true;
    }
    d.kind == Kind::Printer && p == 9100
}

/// SSH-version om SSH är påslaget (nyckel, hostname och domännamn), annars None
pub fn ssh_enabled(d: &Device) -> Option<String> {
    let c = d.config.as_ref()?;
    let on = c.crypto_key.map(|k| k != 0).unwrap_or(false)
        && !c.hostname.is_empty()
        && c.hostname != "Router"
        && c.hostname != "Switch"
        && c.domain_name.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if on {
        Some(c.ssh.version.filter(|v| *v != 0).map(|v| v.to_string()).unwrap_or_else(|| "1.99".into()))
    } else {
        None
    }
}

pub fn vty_allows(d: &Device, what: &str) -> bool {
    let l = &d.cfg().lines;
    let ok = |line: &Line| line.transport == "all" || line.transport == what || line.transport == "telnet ssh" || line.transport == "ssh telnet";
    ok(&l.vty0_4) || ok(&l.vty5_15)
}

/// Ping tur och retur. Returnerar resultat utan slump (loss anges separat).
pub fn ping_internal(st: &mut State, d: &Derived, from: &str, dst: &str, opts: &PingOpts) -> PingResult {
    let proto = opts.proto.clone().unwrap_or_else(|| "icmp".into());
    let pkt = Pkt { dst: dst.to_string(), src: opts.src.clone(), proto: proto.clone(), dport: opts.dport, ..Default::default() };
    let out = send(st, d, from, pkt, opts);
    if !out.ok {
        return PingResult {
            ok: false,
            reason: out.reason,
            where_: out.where_,
            from_ip: out.from_ip,
            hops: out.hops,
            local: out.local,
            iface: out.iface,
            mtu: out.mtu,
            ipsec: out.ipsec,
            ..Default::default()
        };
    }
    let at = out.at.clone().unwrap_or_default();
    if !listens(st, &at, &proto, opts.dport) {
        return PingResult { ok: false, reason: Some("refused".into()), at: Some(at), hops: out.hops, ..Default::default() };
    }
    // Svaret
    let back_opts = PingOpts { learn_arp: opts.learn_arp, ..Default::default() };
    let back = if at.starts_with("INTERNET:") {
        send_from_internet(st, d, out.pkt.dst.clone(), out.pkt.src.clone().unwrap_or_default(), &proto, out.pkt.nat_local.clone())
    } else {
        let bp = Pkt { src: Some(out.pkt.dst.clone()), dst: out.pkt.src.clone().unwrap_or_default(), proto: proto.clone(), nat_local: out.pkt.nat_local.clone(), ..Default::default() };
        send(st, d, &at, bp, &back_opts)
    };
    if !back.ok {
        return PingResult { ok: false, reason: Some("noreply".into()), back_reason: back.reason.clone(), hops: out.hops, at: Some(at), ..Default::default() };
    }
    let loss = 1.0 - (1.0 - out.loss) * (1.0 - back.loss);
    let ttl = if at.starts_with("INTERNET:") {
        56 - out.hops.len() as i64
    } else {
        let rd = st.dev(&at);
        let base = if rd.os == Os::Ios { 255 } else if rd.os == Os::Linux { 64 } else { 128 };
        base - out.hops.iter().filter(|h| h.dev != at).count() as i64
    };
    PingResult {
        ok: true,
        loss,
        hops: out.hops.clone(),
        at: Some(at),
        ttl,
        reply_from: Some(out.pkt.dst.clone()),
        src_used: out.pkt.src.clone(),
        tunnel: out.pkt.tunnel || back.pkt.tunnel,
        mss: out.pkt.mss.unwrap_or(65535).min(back.pkt.mss.unwrap_or(65535)),
        ..Default::default()
    }
}

/// Stor TCP-överföring (filkopiering, webbsida). Genom en tunnel utan adjust-mss
/// fastnar de fulla paketen: handskakningen går igenom men överföringen hänger.
pub fn big_transfer(st: &mut State, from: &str, dst: &str, dport: i64) -> BigTransfer {
    let d = get(st);
    let r = ping_internal(st, &d, from, dst, &PingOpts { proto: Some("tcp".into()), dport: Some(dport), learn_arp: true, ..Default::default() });
    if !r.ok {
        return BigTransfer { ok: false, stage: Some("connect".into()), res: r, mtu: None };
    }
    if r.tunnel && r.mss > TUNNEL_MTU - 40 {
        return BigTransfer { ok: false, stage: Some("transfer".into()), res: r, mtu: Some(TUNNEL_MTU) };
    }
    BigTransfer { ok: true, stage: None, res: r, mtu: None }
}

// ---------------------------------------------------------------- Lastbalanserare

fn lb_member_up(st: &mut State, d: &Derived, lb_id: &str, pool: &LbPool, m: &LbMember) -> bool {
    if !m.enabled {
        return false;
    }
    if pool.monitor == "icmp" {
        return ping_internal(st, d, lb_id, &m.ip, &PingOpts::default()).ok;
    }
    ping_internal(st, d, lb_id, &m.ip, &PingOpts::tcp(m.port)).ok
}

pub fn lb_status(st: &mut State, lb: &str) -> Vec<LbPoolStatus> {
    let d = get(st);
    let pools: Vec<(String, LbPool)> = st.dev(lb).lb.as_ref().map(|l| l.pools.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    let mut out = vec![];
    for (n, p) in pools {
        let mut members = vec![];
        for m in p.members.iter() {
            let s = st.dev(lb).lb.as_ref().and_then(|l| l.stats.get(&m.ip)).cloned().unwrap_or_default();
            let up = lb_member_up(st, &d, lb, &p, m);
            members.push(LbMemberStatus { m: m.clone(), up, req: s.req, fail: s.fail });
        }
        out.push(LbPoolStatus { name: n, pool: p, members });
    }
    out
}

/// En HTTP-förfrågan från en klient till en adress. Går den till en lastbalanserare
/// väljs en server med round robin bland dem som hälsokontrollen säger är uppe.
pub fn http_get(st: &mut State, from: &str, ip: &str, port: i64) -> HttpResult {
    let d = get(st);
    let port = if port == 0 { 80 } else { port };
    let r = ping_internal(st, &d, from, ip, &PingOpts { proto: Some("tcp".into()), dport: Some(port), learn_arp: true, ..Default::default() });
    if !r.ok {
        return HttpResult { ok: false, code: 0, server: None, via: None, member: None, res: r };
    }
    let at = r.at.clone().unwrap_or_default();
    let lb_id = match st.devices.get(&at) {
        Some(dd) if dd.kind == Kind::Lb => dd.id.clone(),
        _ => return HttpResult { ok: true, code: 200, server: Some(at), via: None, member: None, res: r },
    };
    let pool = {
        let dd = st.dev(&lb_id);
        lb_pool(dd, r.reply_from.as_deref()).or_else(|| lb_pool(dd, None)).cloned()
    };
    let pool = match pool {
        Some(p) => p,
        None => return HttpResult { ok: false, code: 503, server: None, via: Some(lb_id), member: None, res: r },
    };
    let mut up = vec![];
    for m in pool.members.iter() {
        if lb_member_up(st, &d, &lb_id, &pool, m) {
            up.push(m.clone());
        }
    }
    if up.is_empty() {
        return HttpResult { ok: false, code: 503, server: None, via: Some(lb_id), member: None, res: r };
    }
    let m = {
        let lb = st.dev_mut(&lb_id).lb.as_mut().unwrap();
        let m = up[lb.rr % up.len()].clone();
        lb.rr = (lb.rr + 1) % 1000;
        lb.stats.entry(m.ip.clone()).or_default().req += 1;
        m
    };
    let r2 = ping_internal(st, &d, &lb_id, &m.ip, &PingOpts::tcp(m.port));
    if !r2.ok {
        st.dev_mut(&lb_id).lb.as_mut().unwrap().stats.entry(m.ip.clone()).or_default().fail += 1;
        return HttpResult { ok: false, code: 502, server: None, via: Some(lb_id), member: Some(m), res: r };
    }
    HttpResult { ok: true, code: 200, server: r2.at.clone(), via: Some(lb_id), member: Some(m), res: r }
}

fn send_from_internet(st: &mut State, d: &Derived, src: String, dst: String, proto: &str, nat_local: Option<String>) -> SendResult {
    let fail = || SendResult { ok: false, reason: Some("drop".into()), loss: 0.0, ..Default::default() };
    // Internet skickar tillbaka via den operatör som äger adressen
    if U::is_private(&dst) {
        return fail();
    }
    let isp_id = isp_for(st, &dst, None).unwrap_or_else(|| "ISP".into());
    let ep = match d.eps.get(&ek(&isp_id, "nic")) {
        Some(e) if e.up => e.clone(),
        _ => return fail(),
    };
    let seg = match d.comp.get(&ep.key) {
        Some(s) => *s,
        None => return fail(),
    };
    let owner = match arp_owner(st, d, seg, &dst, &ep.key) {
        Some(o) => o,
        None => return fail(),
    };
    // Fortsätt in i routern som om paketet kom in där
    let mut pkt = Pkt { src: Some(src), dst, proto: proto.to_string(), ttl: 60, nat_local, ..Default::default() };
    send_ingress(st, d, &owner, &mut pkt)
}

/// Liten variant av send som börjar med ett paket som redan anlänt till en enhet
fn send_ingress(st: &mut State, d: &Derived, ingress: &Ep, pkt: &mut Pkt) -> SendResult {
    let dev = ingress.dev.clone();
    let res = |ok: bool, loss: f64, reason: Option<&str>| SendResult { ok, loss, reason: reason.map(|s| s.to_string()), ..Default::default() };
    if !is_router(st.dev(&dev)) {
        return res(ingress.ip.as_deref() == Some(pkt.dst.as_str()), 0.0, None);
    }
    let (acl_in, nat_dir) = {
        let i = st.dev(&dev).ifc(&ingress.iface);
        (i.and_then(|i| i.acl_in.clone()), i.and_then(|i| i.nat_dir))
    };
    if !acl_check(st, &dev, acl_in.as_deref(), pkt) {
        return res(false, 0.0, Some("acl"));
    }
    if nat_dir == Some(NatDir::Outside) {
        if let Some(tr) = nat_lookup_global(st, &dev, &pkt.dst, pkt.nat_local.as_deref()) {
            pkt.dst = tr;
        }
    }
    if router_owns(d, &dev, &pkt.dst) {
        return res(true, 0.0, None);
    }
    let rt = match route_lookup(st, d, &dev, &pkt.dst) {
        Some(r) => r,
        None => return res(false, 0.0, Some("net-unreachable")),
    };
    let acl_out = st.dev(&dev).ifc(&rt.iface).and_then(|o| o.acl_out.clone());
    if !acl_check(st, &dev, acl_out.as_deref(), pkt) {
        return res(false, 0.0, Some("acl"));
    }
    let nh = rt.nh.clone().unwrap_or_else(|| pkt.dst.clone());
    let seg = match d.comp.get(&rt.ep_key) {
        Some(s) => *s,
        None => return res(false, 0.0, Some("arp")),
    };
    let owner = match arp_owner(st, d, seg, &nh, &rt.ep_key) {
        Some(o) => o,
        None => return res(false, 0.0, Some("arp")),
    };
    let loss = l2_loss(d, &rt.ep_key, &owner.key);
    if st.dev(&owner.dev).kind == Kind::Hub {
        return res(false, 0.0, None);
    }
    if owner.ip.as_deref() == Some(pkt.dst.as_str()) && !is_router(st.dev(&owner.dev)) {
        return res(true, loss, None);
    }
    let more = send_ingress(st, d, &owner, pkt);
    SendResult { ok: more.ok, loss: 1.0 - (1.0 - loss) * (1.0 - more.loss), reason: more.reason, ..Default::default() }
}

pub fn ping(st: &mut State, from: &str, dst: &str, mut opts: PingOpts) -> PingResult {
    let d = get(st);
    opts.learn_arp = true;
    ping_internal(st, &d, from, dst, &opts)
}

/// (framåt-resultatet, hela pingen)
pub fn traceroute(st: &mut State, from: &str, dst: &str, src: Option<&str>) -> (SendResult, PingResult) {
    let d = get(st);
    let pkt = Pkt { dst: dst.to_string(), src: src.map(|s| s.to_string()), ..Default::default() };
    let out = send(st, &d, from, pkt, &PingOpts::default());
    let r = ping_internal(st, &d, from, dst, &PingOpts { src: src.map(|s| s.to_string()), ..Default::default() });
    (out, r)
}

// ---------------------------------------------------------------- DHCP

pub fn dhcp(st: &mut State, host: &str) -> Lease {
    let d = get(st);
    let mac = st.dev(host).nic().mac.clone();
    let apipa = || {
        let x = U::hash(&mac);
        Lease { ip: format!("169.254.{}.{}", 1 + (x % 254), 1 + ((x >> 8) % 254)), mask: "255.255.0.0".into(), gw: None, dns: vec![], apipa: true, ..Default::default() }
    };
    let me = match d.eps.get(&ek(host, "nic")) {
        Some(m) if m.up => m.clone(),
        _ => {
            let mut l = apipa();
            l.nolink = true;
            st.dev_mut(host).lease = Some(l.clone());
            touch(st);
            return l;
        }
    };
    let seg = d.comp.get(&me.key).copied().unwrap_or(usize::MAX);
    let mut servers: Vec<Ep> = d.seg_eps(seg).into_iter().filter(|e| e.up && is_router(st.dev(&e.dev))).cloned().collect();
    let chain_loss = l2_loss(&d, &me.key, servers.first().map(|s| s.key.as_str()).unwrap_or(&me.key));
    if chain_loss >= 1.0 {
        servers.clear();
    }
    for s in servers {
        let rid = s.dev.clone();
        let acl_in = st.dev(&rid).ifc(&s.iface).and_then(|i| i.acl_in.clone());
        let ap = Pkt { src: Some("0.0.0.0".into()), dst: "255.255.255.255".into(), proto: "udp".into(), dport: Some(67), ..Default::default() };
        if !acl_check(st, &rid, acl_in.as_deref(), &ap) {
            continue;
        }
        let sip = s.ip.clone().unwrap_or_default();
        let found = st
            .dev(&rid)
            .cfg()
            .dhcp_pools
            .iter()
            .find(|(_, pl)| match (&pl.network, &pl.mask) {
                (Some(n), Some(m)) => U::in_net(&sip, n, m),
                _ => false,
            })
            .map(|(n, p)| (n.clone(), p.clone()));
        let (pool_name, pool) = match found {
            Some(x) => x,
            None => continue,
        };
        let ip = match allocate(st, &d, &rid, &pool, &pool_name, host, seg) {
            Some(ip) => ip,
            None => {
                push_log(st, &rid, &format!("%DHCPD-4-NO_FREE_ADDRESS: No free address in pool {} for client {}.", pool_name, U::mac_dots(&mac)));
                continue;
            }
        };
        let lease = Lease {
            ip,
            mask: pool.mask.clone().unwrap_or_default(),
            gw: pool.default_router.first().cloned(),
            dns: pool.dns.clone(),
            server: Some(sip),
            pool: Some(pool_name),
            obtained: st.time,
            apipa: false,
            nolink: false,
        };
        st.dev_mut(host).lease = Some(lease.clone());
        touch(st);
        return lease;
    }
    let l = apipa();
    st.dev_mut(host).lease = Some(l.clone());
    touch(st);
    l
}

fn excluded(r: &Device, ip: &str) -> bool {
    let n = U::ipn(ip);
    r.cfg().dhcp_excluded.iter().any(|(a, b)| {
        let hi = if b.is_empty() { a } else { b };
        n >= U::ipn(a) && n <= U::ipn(hi)
    })
}

fn allocate(st: &mut State, d: &Derived, rid: &str, pool: &Pool, pool_name: &str, host: &str, seg: usize) -> Option<String> {
    let mac = st.dev(host).nic().mac.clone();
    let (pn, pm) = (pool.network.clone().unwrap_or_default(), pool.mask.clone().unwrap_or_default());
    for (ip, b) in st.dev(rid).rt.dhcp_bindings.iter() {
        if b.mac.as_deref() == Some(mac.as_str()) && b.pool == pool_name && U::in_net(ip, &pn, &pm) {
            return Some(ip.clone());
        }
    }
    let net = U::ipn(&U::network(&pn, &pm));
    let bc = U::ipn(&U::broadcast(&pn, &pm));
    let time = st.time;
    let mut n = net.wrapping_add(1);
    while n < bc {
        let cand = U::int_to_ip(n);
        n += 1;
        if excluded(st.dev(rid), &cand) {
            continue;
        }
        if st.dev(rid).rt.dhcp_bindings.contains_key(&cand) {
            continue;
        }
        if dev_eps(d, rid).iter().any(|e| e.ip.as_deref() == Some(cand.as_str())) {
            continue;
        }
        let in_use = d.seg_eps(seg).iter().any(|e| e.up && e.ip.as_deref() == Some(cand.as_str()) && e.dev != host);
        if in_use {
            st.dev_mut(rid).rt.dhcp_bindings.insert(cand.clone(), Binding { mac: None, pool: pool_name.into(), conflict: true, t: time, host: None });
            push_log(st, rid, &format!("%DHCPD-4-PING_CONFLICT: DHCP address conflict:  server pinged {}.", cand));
            continue;
        }
        st.dev_mut(rid).rt.dhcp_bindings.insert(cand.clone(), Binding { mac: Some(mac), pool: pool_name.into(), conflict: false, t: time, host: Some(host.into()) });
        return Some(cand);
    }
    None
}

pub fn release(st: &mut State, host: &str) {
    let mac = st.dev(host).nic().mac.clone();
    for d in st.devices.values_mut() {
        if !is_router(d) {
            continue;
        }
        d.rt.dhcp_bindings.retain(|_, b| b.mac.as_deref() != Some(mac.as_str()));
    }
    st.dev_mut(host).lease = None;
    touch(st);
}

// ---------------------------------------------------------------- DNS

pub fn resolve(st: &mut State, host: &str, name: &str) -> Resolved {
    let name = name.to_lowercase();
    let ok = |ip: &str, server: Option<String>| Resolved { ip: Some(ip.to_string()), error: None, server };
    let err = |e: &str, server: Option<String>| Resolved { ip: None, error: Some(e.to_string()), server };
    if U::is_ip(&name) {
        return ok(&name, None);
    }
    let d = get(st);
    let h = st.dev(host);
    let dns_list: Vec<String>;
    if h.os == Os::Ios {
        if let Some(ip) = h.cfg().hosts.get(&name) {
            return ok(ip, None);
        }
        if !h.cfg().domain_lookup {
            return err("nolookup", None);
        }
        let mut l = h.cfg().name_servers.clone();
        if l.is_empty() {
            l = vec!["255.255.255.255".into()];
        }
        dns_list = l;
    } else {
        dns_list = host_ip_conf(h).map(|c| c.dns).unwrap_or_default();
    }
    if dns_list.is_empty() {
        return err("noserver", None);
    }
    for srv in dns_list.iter() {
        let r = ping_internal(st, &d, host, srv, &PingOpts::udp(53));
        if !r.ok {
            continue;
        }
        let at = r.at.clone().unwrap_or_default();
        if at.starts_with("INTERNET:") {
            return match public_dns(&name) {
                Some(ip) => ok(ip, Some(srv.clone())),
                None => err("nxdomain", Some(srv.clone())),
            };
        }
        let sd = st.dev(&at);
        if sd.os == Os::Ios && sd.cfg().dns_server {
            let short = name.strip_suffix(".nordvik.example").unwrap_or(&name).to_string();
            if let Some(ip) = sd.cfg().hosts.get(&short) {
                return ok(ip, Some(srv.clone()));
            }
            if sd.cfg().domain_lookup && !sd.cfg().name_servers.is_empty() {
                let ns = sd.cfg().name_servers[0].clone();
                let sid = sd.id.clone();
                let f = ping_internal(st, &d, &sid, &ns, &PingOpts::udp(53));
                if f.ok {
                    return match public_dns(&name) {
                        Some(ip) => ok(ip, Some(srv.clone())),
                        None => err("nxdomain", Some(srv.clone())),
                    };
                }
                return err("servfail", Some(srv.clone()));
            }
            return err("nxdomain", Some(srv.clone()));
        }
    }
    err("timeout", dns_list.first().cloned())
}

// ---------------------------------------------------------------- Klocka och loggar

const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAY: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn two(n: u32) -> String {
    format!("{:02}", n)
}

pub fn ntp_synced(st: &mut State, dev: &str) -> bool {
    let (server, src) = {
        let c = match st.dev(dev).config.as_ref() {
            Some(c) => c,
            None => return true,
        };
        if c.ntp_servers.is_empty() {
            return false;
        }
        let src = c.ntp_source.as_ref().and_then(|s| c.ifaces.get(s)).and_then(|i| i.ip.as_ref()).map(|ip| ip.addr.clone());
        (c.ntp_servers[0].clone(), src)
    };
    if let Some(v) = st.ntp_cache.get(dev) {
        return *v;
    }
    // Under pågående beräkning finns ingen härledd data än: då räknas klockan som synkad
    let d = match st.cache.clone() {
        Some(c) => c,
        None => return true,
    };
    let ok = ping_internal(st, &d, dev, &server, &PingOpts { proto: Some("udp".into()), dport: Some(123), src, ..Default::default() }).ok;
    st.ntp_cache.insert(dev.to_string(), ok);
    ok
}

fn start_ms() -> f64 {
    U::date_utc(2026, 8, 29, 6, 0, 0) // 08:00 svensk tid (CEST)
}

pub fn device_clock(st: &mut State, dev: &str) -> Clock {
    let synced = if st.dev(dev).os == Os::Ios { ntp_synced(st, dev) } else { true };
    let d = st.dev(dev);
    let ms = if synced {
        start_ms() + st.time * 1000.0 + 2.0 * 3600.0 * 1000.0
    } else if let Some(cs) = &d.rt.clock_set {
        cs.ms + (st.time - cs.at) * 1000.0
    } else {
        U::date_utc(1993, 2, 1, 0, 0, 0) + (st.time - d.rt.boot_time) * 1000.0
    };
    let t = U::date_parts(ms);
    Clock {
        synced,
        ms,
        mon: MON[t.mon0 as usize],
        day: t.day,
        dow: DAY[t.dow as usize],
        year: t.year,
        hms: format!("{}:{}:{}", two(t.h), two(t.m), two(t.s)),
        msec: format!("{:03}", (t.ms as u64 + U::now_ms()) % 1000),
    }
}

pub fn stamp(st: &mut State, dev: &str) -> String {
    let (msec, boot) = {
        let d = st.dev(dev);
        (d.config.as_ref().map(|c| c.timestamps_msec).unwrap_or(false), d.rt.boot_time)
    };
    if !msec {
        let up = ((st.time - boot).floor()).max(0.0) as u64;
        let dd = up / 86400;
        let hh = (up % 86400) / 3600;
        let mm = (up % 3600) / 60;
        let ss = up % 60;
        return if dd > 0 { format!("{}d{}h", dd, two(hh as u32)) } else { format!("{}:{}:{}", two(hh as u32), two(mm as u32), two(ss as u32)) };
    }
    let c = device_clock(st, dev);
    format!("{}{} {}{} {}.{}", if c.synced { "" } else { "*" }, c.mon, if c.day < 10 { " " } else { "" }, c.day, c.hms, c.msec)
}

pub fn push_log(st: &mut State, dev: &str, msg: &str) {
    match st.devices.get(dev) {
        Some(d) if d.os == Os::Ios => {}
        _ => return,
    }
    let line = format!("{}: {}", stamp(st, dev), msg);
    let d = st.dev_mut(dev);
    d.rt.logs.push(line.clone());
    if d.rt.logs.len() > 400 {
        d.rt.logs.remove(0);
    }
    st.log_events.push((dev.to_string(), line));
}

/// Jämför portstatus före och efter en ändring och skriver loggrader
fn status_of(st: &State, d: &Derived, dev: &str, port: &str) -> &'static str {
    let dd = st.dev(dev);
    let i = match dd.ifc(port) {
        Some(i) => i,
        None => return "none",
    };
    if i.shutdown {
        return "admin";
    }
    if dd.rt.errdisabled.get(port).map(|s| !s.is_empty()).unwrap_or(false) {
        return "err";
    }
    if d.ports.get(&key(dev, port)).map(|p| p.up).unwrap_or(false) { "up" } else { "down" }
}

/// Räknar om och skriver loggrader för portar som ändrat status.
pub fn refresh(st: &mut State) -> Rc<Derived> {
    let before = st.port_snap.clone();
    touch(st);
    let d = get(st);
    let mut snap: HashMap<String, String> = HashMap::new();
    let mut logs: Vec<(String, String)> = vec![];
    for (id, dev) in st.devices.iter() {
        if dev.os != Os::Ios {
            continue;
        }
        for (n, i) in dev.cfg().ifaces.iter() {
            if i.internal {
                continue;
            }
            let s: &str = if i.is_loop {
                if i.shutdown { "admin" } else { "up" }
            } else if i.svi.is_some() || i.parent.is_some() {
                if i.shutdown {
                    "admin"
                } else if d.eps.get(&ek(id, n)).map(|e| e.up).unwrap_or(false) {
                    "up"
                } else {
                    "down"
                }
            } else {
                status_of(st, &d, id, n)
            };
            let k = key(id, n);
            snap.insert(k.clone(), s.to_string());
            let b = before.get(&k).map(|s| s.as_str());
            // Ett nytt loopback-interface loggas när det kommer upp, som på en riktig router
            if b.is_none() && i.is_loop && s == "up" && !before.is_empty() {
                logs.push((id.clone(), format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {}, changed state to up", n)));
                continue;
            }
            let b = match b {
                Some(b) if b != s => b,
                _ => continue,
            };
            if i.parent.is_some() {
                continue;
            }
            if s == "up" {
                if i.svi.is_none() {
                    logs.push((id.clone(), format!("%LINK-3-UPDOWN: Interface {}, changed state to up", n)));
                }
                logs.push((id.clone(), format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {}, changed state to up", n)));
            } else if b == "up" {
                logs.push((id.clone(), format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {}, changed state to down", n)));
                if i.svi.is_none() {
                    logs.push((
                        id.clone(),
                        if s == "admin" { format!("%LINK-5-CHANGED: Interface {}, changed state to administratively down", n) } else { format!("%LINK-3-UPDOWN: Interface {}, changed state to down", n) },
                    ));
                }
            } else if s == "admin" && b != "admin" && i.svi.is_none() {
                logs.push((id.clone(), format!("%LINK-5-CHANGED: Interface {}, changed state to administratively down", n)));
            }
        }
    }
    for (id, m) in logs {
        push_log(st, &id, &m);
    }
    st.port_snap = snap;
    d
}

/// Tiden går: räknare, flappande portar, periodiska loggar.
pub fn tick(st: &mut State, dt: f64) {
    let prev_t = st.time;
    st.time += dt;
    let d = get(st);
    let t = st.time;
    let link_state: HashMap<String, LinkState> = st.links.iter().map(|l| (l.id.clone(), l.state)).collect();
    // Räknare
    let mut keys: Vec<&String> = d.ports.keys().collect();
    keys.sort();
    for k in keys {
        let p = &d.ports[k];
        let (dn, pn) = match k.split_once('|') {
            Some(x) => x,
            None => continue,
        };
        match st.devices.get(dn) {
            Some(dev) if dev.os == Os::Ios => {}
            _ => continue,
        }
        if !p.up {
            st.dev_mut(dn).rt.counters.entry(pn.to_string()).or_default();
            continue;
        }
        let rate = if !d.storm.is_empty() { 90000.0 } else { 40.0 };
        let r1 = st.random();
        let r2 = st.random();
        let c = st.dev_mut(dn).rt.counters.entry(pn.to_string()).or_default();
        c.inp += rate * dt * (0.8 + r1 * 0.4);
        c.outp += rate * dt * (0.8 + r2 * 0.4);
        if p.mismatch {
            if p.neg.as_ref().map(|n| n.duplex == "half").unwrap_or(false) {
                c.late += dt * 0.6;
                c.coll += dt * 2.0;
            } else {
                c.crc += dt * 1.1;
                c.runts += dt * 0.4;
            }
        }
        if link_state.get(&p.link) == Some(&LinkState::Flapping) {
            c.crc += dt * 0.2;
        }
    }
    // Flappande kablar: länken går ner och upp
    let links = st.links.clone();
    for l in links.iter() {
        if l.state != LinkState::Flapping {
            continue;
        }
        let period = 23.0;
        if (prev_t / period).floor() != (t / period).floor() {
            for s in [&l.a, &l.b] {
                let dv = st.dev(&s.dev);
                if dv.os != Os::Ios || dv.ifc(&s.port).map(|i| i.shutdown).unwrap_or(true) {
                    continue;
                }
                for m in [
                    format!("%LINK-3-UPDOWN: Interface {}, changed state to down", s.port),
                    format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {}, changed state to down", s.port),
                    format!("%LINK-3-UPDOWN: Interface {}, changed state to up", s.port),
                    format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {}, changed state to up", s.port),
                ] {
                    push_log(st, &s.dev, &m);
                }
                if let Some(c) = st.dev_mut(&s.dev).rt.counters.get_mut(&s.port) {
                    c.flaps += 1.0;
                }
            }
        }
    }
    // Periodiska meddelanden var 60:e sekund
    if (prev_t / 60.0).floor() != (t / 60.0).floor() {
        for l in links.iter() {
            let info = match d.links.get(&l.id) {
                Some(i) if i.mismatch => i,
                _ => continue,
            };
            let (a, b) = (st.dev(&l.a.dev), st.dev(&l.b.dev));
            if a.os == Os::Ios && b.os == Os::Ios {
                let (na, nb) = (info.neg_a.clone().unwrap(), info.neg_b.clone().unwrap());
                let (ha, hb) = (a.cfg().hostname.clone(), b.cfg().hostname.clone());
                push_log(st, &l.a.dev, &format!("%CDP-4-DUPLEX_MISMATCH: duplex mismatch discovered on {} (not {} duplex), with {} {} ({} duplex).", l.a.port, na.duplex, hb, l.b.port, nb.duplex));
                push_log(st, &l.b.dev, &format!("%CDP-4-DUPLEX_MISMATCH: duplex mismatch discovered on {} (not {} duplex), with {} {} ({} duplex).", l.b.port, nb.duplex, ha, l.a.port, na.duplex));
            }
        }
        for nm in d.native_mismatch.iter() {
            let ha = st.dev(&nm.a.dev).cfg().hostname.clone();
            let hb = st.dev(&nm.b.dev).cfg().hostname.clone();
            push_log(st, &nm.a.dev, &format!("%CDP-4-NATIVE_VLAN_MISMATCH: Native VLAN mismatch discovered on {} ({}), with {} {} ({}).", nm.a.port, nm.na, hb, nm.b.port, nm.nb));
            push_log(st, &nm.a.dev, &format!("%SPANTREE-2-RECV_PVID_ERR: Received BPDU with inconsistent peer vlan id {} on {} VLAN{}.", nm.nb, nm.a.port, nm.na));
            push_log(st, &nm.a.dev, &format!("%SPANTREE-2-BLOCK_PVID_LOCAL: Blocking {} on VLAN{:04}. Inconsistent local vlan.", nm.a.port, nm.na));
            push_log(st, &nm.b.dev, &format!("%CDP-4-NATIVE_VLAN_MISMATCH: Native VLAN mismatch discovered on {} ({}), with {} {} ({}).", nm.b.port, nm.nb, ha, nm.a.port, nm.na));
            push_log(st, &nm.b.dev, &format!("%SPANTREE-2-RECV_PVID_ERR: Received BPDU with inconsistent peer vlan id {} on {} VLAN{}.", nm.na, nm.b.port, nm.nb));
        }
    }
    if (prev_t / 7.0).floor() != (t / 7.0).floor() {
        for (v, list) in d.storm.iter() {
            for (idx, id) in list.iter().enumerate() {
                if idx > 1 {
                    continue;
                }
                let victim = st.devices.get("PC-Anna").and_then(|p| p.nic.as_ref()).map(|n| n.mac.clone());
                if let Some(vm) = victim {
                    push_log(st, id, &format!("%SW_MATM-4-MACFLAP_NOTIF: Host {} in vlan {} is flapping between port Gi0/24 and port Gi0/25", U::mac_dots(&vm), v));
                }
            }
        }
    }
}

/// Trafikbelastning i Mbit/s för övervakningsgrafen
pub fn link_load(st: &mut State, dev: &str, port: &str) -> f64 {
    let d = get(st);
    let p = match d.ports.get(&key(dev, port)) {
        Some(p) if p.up => p,
        _ => return 0.0,
    };
    if !d.storm.is_empty() {
        return 940.0 + st.random() * 50.0;
    }
    let flapping = st.links.iter().any(|l| l.id == p.link && l.state == LinkState::Flapping);
    if flapping {
        let ph = st.time % 23.0;
        if ph < 2.5 {
            return 0.0;
        }
        return 30.0 + ph * 4.0 + st.random() * 8.0;
    }
    let mut base = 60.0 + 40.0 * (st.time / 40.0).sin() + st.random() * 20.0;
    if p.mismatch {
        base *= 0.3;
    }
    base
}

// ------------------------------------------------------------------ Resten av API:t (fanns redan)

pub fn vlan_exists(sw: &Device, v: u16) -> bool {
    let c = sw.cfg();
    c.vlans.contains_key(&v) && c.vlan_state.get(&v).map(|s| s != "suspend").unwrap_or(true)
}
pub fn bridge_id(sw: &Device, v: u16) -> BridgeId {
    let pr = sw.cfg().stp_priority.get(&v).copied().unwrap_or(32768) + v as u32;
    BridgeId { pr, mac: sw.base_mac.clone() }
}
pub fn dev_eps<'a>(d: &'a Derived, dev: &str) -> Vec<&'a Ep> {
    d.eps.values().filter(|e| e.dev == dev).collect()
}
pub fn clear_crypto(st: &mut State, dev: &str) {
    let d = st.dev_mut(dev);
    d.rt.ike.clear();
    d.rt.ipsec.clear();
    d.rt.p2fail.clear();
}
pub fn lb_pool<'a>(d: &'a Device, vip: Option<&str>) -> Option<&'a LbPool> {
    let lb = d.lb.as_ref()?;
    lb.pools.values().find(|p| vip.map(|v| p.vip == v).unwrap_or(true))
}
/// Normaliserad adress för spegelkontroll av crypto-ACL:er ("ip/wild")
pub fn spec_norm(s: Option<&AddrSpec>) -> String {
    match s {
        None => "0.0.0.0/255.255.255.255".into(),
        Some(s) if s.any => "0.0.0.0/255.255.255.255".into(),
        Some(s) => {
            if let Some(h) = &s.host {
                return format!("{}/0.0.0.0", h);
            }
            let w = s.wild.clone().unwrap_or_else(|| "0.0.0.0".into());
            let ip = U::int_to_ip(U::ipn(s.ip.as_deref().unwrap_or("0.0.0.0")) & !U::ipn(&w));
            format!("{}/{}", ip, w)
        }
    }
}
pub fn mirrors(a: &Ace, b: &Ace) -> bool {
    a.action == "permit"
        && b.action == "permit"
        && a.proto.as_deref().unwrap_or("ip") == b.proto.as_deref().unwrap_or("ip")
        && spec_norm(a.src.as_ref()) == spec_norm(b.dst.as_ref())
        && spec_norm(a.dst.as_ref()) == spec_norm(b.src.as_ref())
}
