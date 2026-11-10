//! Nätverkssimulatorn. Allt härleds ur konfigurationen och den fysiska kopplingen,
//! så att ett fel bara försvinner när orsaken faktiskt är rättad.
//! Port av js/net/sim.js. Typerna och signaturerna här är det gemensamma API:t för resten av spelet.
#![allow(unused_variables, dead_code)]
use super::dynroute::DynOut;
use super::model::*;
use super::util::{self as U, Rng};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
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

// ------------------------------------------------------------------ API (implementeras i porten av sim.js)

/// Räknar om allt härlett (compute i sim.js) och lägger det i state.cache.
pub fn compute(st: &mut State) -> Rc<Derived> {
    todo!("sim::compute")
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
/// Räknar om och skriver loggrader för portar som ändrat status.
pub fn refresh(st: &mut State) -> Rc<Derived> {
    todo!("sim::refresh")
}
/// Tiden går: räknare, flappande portar, periodiska loggar.
pub fn tick(st: &mut State, dt: f64) {
    todo!("sim::tick")
}
pub fn ping(st: &mut State, from: &str, dst: &str, opts: PingOpts) -> PingResult {
    todo!("sim::ping")
}
pub fn ping_internal(st: &mut State, d: &Derived, from: &str, dst: &str, opts: &PingOpts) -> PingResult {
    todo!("sim::ping_internal")
}
/// (framåt-resultatet, hela pingen)
pub fn traceroute(st: &mut State, from: &str, dst: &str, src: Option<&str>) -> (SendResult, PingResult) {
    todo!("sim::traceroute")
}
pub fn dhcp(st: &mut State, host: &str) -> Lease {
    todo!("sim::dhcp")
}
pub fn release(st: &mut State, host: &str) {
    todo!("sim::release")
}
pub fn resolve(st: &mut State, host: &str, name: &str) -> Resolved {
    todo!("sim::resolve")
}
pub fn installed_routes(st: &State, d: &Derived, dev: &str) -> Vec<InstalledRoute> {
    todo!("sim::installed_routes")
}
pub fn route_lookup(st: &State, d: &Derived, dev: &str, dst: &str) -> Option<RouteHit> {
    todo!("sim::route_lookup")
}
/// "access" eller "trunk"
pub fn op_mode(st: &State, dev: &str, port: &str) -> &'static str {
    todo!("sim::op_mode")
}
pub fn trunk_vlans(sw: &Device, i: &Iface) -> Vec<u16> {
    todo!("sim::trunk_vlans")
}
pub fn vlan_exists(sw: &Device, v: u16) -> bool {
    let c = sw.cfg();
    c.vlans.contains_key(&v) && c.vlan_state.get(&v).map(|s| s != "suspend").unwrap_or(true)
}
pub fn host_ip_conf(h: &Device) -> Option<IpConf> {
    todo!("sim::host_ip_conf")
}
/// SSH-version om SSH är påslaget (nyckel, hostname och domännamn), annars None
pub fn ssh_enabled(d: &Device) -> Option<String> {
    todo!("sim::ssh_enabled")
}
pub fn vty_allows(d: &Device, what: &str) -> bool {
    todo!("sim::vty_allows")
}
pub fn push_log(st: &mut State, dev: &str, msg: &str) {
    todo!("sim::push_log")
}
pub fn device_clock(st: &mut State, dev: &str) -> Clock {
    todo!("sim::device_clock")
}
pub fn ntp_synced(st: &mut State, dev: &str) -> bool {
    todo!("sim::ntp_synced")
}
pub fn stamp(st: &mut State, dev: &str) -> String {
    todo!("sim::stamp")
}
/// Trafikbelastning i Mbit/s för övervakningsgrafen
pub fn link_load(st: &mut State, dev: &str, port: &str) -> f64 {
    todo!("sim::link_load")
}
pub fn acl_eval(st: &State, dev: &str, name: &str, pkt: &AclPkt) -> bool {
    todo!("sim::acl_eval")
}
pub fn bridge_id(sw: &Device, v: u16) -> BridgeId {
    let pr = sw.cfg().stp_priority.get(&v).copied().unwrap_or(32768) + v as u32;
    BridgeId { pr, mac: sw.base_mac.clone() }
}
pub fn dev_eps<'a>(d: &'a Derived, dev: &str) -> Vec<&'a Ep> {
    d.eps.values().filter(|e| e.dev == dev).collect()
}
pub fn crypto_status(st: &mut State, dev: &str) -> CryptoStatus {
    todo!("sim::crypto_status")
}
pub fn clear_crypto(st: &mut State, dev: &str) {
    let d = st.dev_mut(dev);
    d.rt.ike.clear();
    d.rt.ipsec.clear();
    d.rt.p2fail.clear();
}
pub fn big_transfer(st: &mut State, from: &str, dst: &str, dport: i64) -> BigTransfer {
    todo!("sim::big_transfer")
}
pub fn http_get(st: &mut State, from: &str, ip: &str, port: i64) -> HttpResult {
    todo!("sim::http_get")
}
pub fn lb_status(st: &mut State, lb: &str) -> Vec<LbPoolStatus> {
    todo!("sim::lb_status")
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
