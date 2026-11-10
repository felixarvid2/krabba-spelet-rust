//! Nordviks nät: enheter, kablar och "golden config" enligt adressplanen (bilaga G).
use super::util::{self as U, VlanList};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Router,
    Switch,
    Host,
    Printer,
    Server,
    Wlc,
    Lb,
    Ap,
    Asa,
    Isp,
    Hub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Os {
    Ios,
    Windows,
    Linux,
    Server,
    Device,
    Wlc,
    Lb,
    Ap,
    Phone,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IpAddr {
    pub addr: String,
    pub mask: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatDir {
    Inside,
    Outside,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortSec {
    pub max: u32,
    pub violation: String,
    pub sticky: bool,
    pub macs: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hsrp {
    pub grp: i64,
    pub pri: i64,
    pub preempt: bool,
    pub ip: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChGroup {
    pub n: i64,
    pub mode: String,
}

/// Ett interface. Fälten täcker switchportar, routerportar, sub-interface, SVI och loopback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Iface {
    pub shutdown: bool,
    pub description: String,
    // Switchport
    pub l2: bool,
    pub mode: String,
    pub access_vlan: u16,
    /// Trunkinkapsling på switchport ("dot1q")
    pub encap: Option<String>,
    pub allowed: Option<VlanList>,
    pub native: u16,
    pub nonegotiate: bool,
    pub speed: String,
    pub duplex: String,
    pub poe: String,
    pub port_sec: Option<PortSec>,
    pub portfast: bool,
    pub sfp: bool,
    pub voice_vlan: Option<i64>,
    // Lager 3
    pub ip: Option<IpAddr>,
    pub nat_dir: Option<NatDir>,
    pub acl_in: Option<String>,
    pub acl_out: Option<String>,
    pub crypto_map: Option<String>,
    pub adjust_mss: Option<i64>,
    pub mtu: Option<i64>,
    pub clock_rate: Option<i64>,
    pub bandwidth: Option<i64>,
    pub ospf_cost: Option<i64>,
    pub ospf_hello: Option<i64>,
    pub ospf_dead: Option<i64>,
    /// Sub-interface: moderporten och VLAN (dot1Q)
    pub parent: Option<String>,
    pub dot1q: Option<u16>,
    pub svi: Option<u16>,
    pub is_loop: bool,
    pub internal: bool,
    pub hsrp: Option<Hsrp>,
    pub ch_group: Option<ChGroup>,
    /// Sparade rader som bara syns i running-config
    pub x: IndexMap<String, String>,
}

impl Iface {
    pub fn switch_port(n: u32) -> Iface {
        Iface {
            l2: true,
            mode: "dynamic auto".into(),
            access_vlan: 1,
            allowed: Some(VlanList::All),
            native: 1,
            speed: "auto".into(),
            duplex: "auto".into(),
            poe: "auto".into(),
            sfp: n > 24,
            ..Default::default()
        }
    }
    pub fn router_port() -> Iface {
        Iface { shutdown: true, speed: "auto".into(), duplex: "auto".into(), ..Default::default() }
    }
    pub fn sub(parent: &str) -> Iface {
        Iface { parent: Some(parent.to_string()), ..Default::default() }
    }
    pub fn svi(v: u16) -> Iface {
        Iface { svi: Some(v), ..Default::default() }
    }
    pub fn loopback() -> Iface {
        Iface { is_loop: true, ..Default::default() }
    }
    pub fn allowed_all(&self) -> bool {
        matches!(self.allowed, Some(VlanList::All) | None)
    }
    pub fn allowed_list(&self) -> Vec<u16> {
        match &self.allowed {
            Some(VlanList::List(l)) => l.clone(),
            _ => vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub net: String,
    pub mask: String,
    pub nh: Option<String>,
    pub iface: Option<String>,
    pub ad: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    pub network: Option<String>,
    pub mask: Option<String>,
    pub default_router: Vec<String>,
    pub dns: Vec<String>,
    /// Lease i dagar, None = infinite
    pub lease: Option<f64>,
    pub opt150: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AddrSpec {
    pub any: bool,
    pub host: Option<String>,
    pub ip: Option<String>,
    pub wild: Option<String>,
}

impl AddrSpec {
    pub fn any() -> Self {
        AddrSpec { any: true, ..Default::default() }
    }
    pub fn net(ip: &str, wild: &str) -> Self {
        AddrSpec { ip: Some(ip.into()), wild: Some(wild.into()), ..Default::default() }
    }
    pub fn host(ip: &str) -> Self {
        AddrSpec { host: Some(ip.into()), ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Ace {
    pub seq: i64,
    pub remark: Option<String>,
    pub action: String,
    pub proto: Option<String>,
    pub src: Option<AddrSpec>,
    pub dst: Option<AddrSpec>,
    pub dport: Option<i64>,
    pub dport2: Option<i64>,
    pub dop: Option<String>,
    pub log: bool,
}

impl Ace {
    pub fn std(seq: i64, action: &str, src: AddrSpec) -> Ace {
        Ace { seq, action: action.into(), src: Some(src), ..Default::default() }
    }
    pub fn ext(seq: i64, action: &str, src: AddrSpec, dst: AddrSpec) -> Ace {
        Ace { seq, action: action.into(), proto: Some("ip".into()), src: Some(src), dst: Some(dst), ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Acl {
    pub extended: bool,
    pub rules: Vec<Ace>,
}

impl Acl {
    pub fn type_str(&self) -> &'static str {
        if self.extended { "extended" } else { "standard" }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NatStatic {
    pub local: String,
    pub global: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NatDyn {
    pub acl: String,
    pub iface: String,
    pub overload: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Nat {
    pub statics: Vec<NatStatic>,
    pub dynamic: Vec<NatDyn>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Ssh {
    pub version: Option<i64>,
    pub timeout: Option<i64>,
    pub retries: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub priv_: i64,
    pub secret: String,
    pub plain: String,
    pub by_game: bool,
    pub pw_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Line {
    pub speed: u32,
    pub transport: String,
    pub login_local: bool,
    pub login: bool,
    pub password: Option<String>,
    pub log_sync: bool,
    pub access_class: Option<String>,
    pub priv_level: Option<i64>,
    pub x: IndexMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lines {
    pub con: Line,
    pub vty0_4: Line,
    pub vty5_15: Line,
}

impl Lines {
    pub fn get_mut(&mut self, n: &str) -> &mut Line {
        match n {
            "con" => &mut self.con,
            "vty0_4" => &mut self.vty0_4,
            _ => &mut self.vty5_15,
        }
    }
    pub fn vty(transport: &str, login_local: bool) -> Line {
        Line { transport: transport.into(), login_local, ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Logging {
    pub buffered: bool,
    pub size: i64,
    pub hosts: Vec<String>,
    pub level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IsakmpPolicy {
    pub enc: String,
    pub hash: String,
    pub auth: String,
    pub group: i64,
    pub lifetime: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformSet {
    pub transforms: Vec<String>,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct CmapEntry {
    pub peer: Option<String>,
    pub ts: Option<String>,
    pub acl: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Crypto {
    pub policies: BTreeMap<i64, IsakmpPolicy>,
    pub keys: IndexMap<String, String>,
    pub transform_sets: IndexMap<String, TransformSet>,
    pub maps: IndexMap<String, BTreeMap<i64, CmapEntry>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OspfNet {
    pub net: String,
    pub wild: String,
    pub area: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ospf {
    pub pid: i64,
    pub router_id: Option<String>,
    pub networks: Vec<OspfNet>,
    pub passive_default: bool,
    pub passive: Vec<String>,
    pub no_passive: Vec<String>,
    pub ref_bw: i64,
    pub default_originate: bool,
    pub default_always: bool,
    pub log_adj: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct BgpNeighbor {
    pub remote_as: Option<i64>,
    pub shutdown: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BgpNet {
    pub net: String,
    pub mask: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bgp {
    pub as_: i64,
    pub router_id: Option<String>,
    pub neighbors: IndexMap<String, BgpNeighbor>,
    pub networks: Vec<BgpNet>,
    pub log_changes: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Vtp {
    pub mode: String,
    pub domain: String,
    pub version: i64,
    pub password: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Snmp {
    pub communities: IndexMap<String, String>,
    pub location: Option<String>,
    pub contact: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Banner {
    pub d: String,
    pub text: String,
}

/// IOS-konfigurationen (running eller startup).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IosConfig {
    pub hostname: String,
    pub enable_secret: Option<String>,
    pub enable_plain: Option<String>,
    pub enable_password: Option<String>,
    pub domain_name: Option<String>,
    pub domain_lookup: bool,
    pub name_servers: Vec<String>,
    pub dns_server: bool,
    pub hosts: IndexMap<String, String>,
    pub ifaces: IndexMap<String, Iface>,
    pub vlans: BTreeMap<u16, String>,
    pub vlan_state: BTreeMap<u16, String>,
    pub stp_off: Vec<u16>,
    pub stp_priority: BTreeMap<u16, u32>,
    pub routes: Vec<Route>,
    pub default_gateway: Option<String>,
    pub dhcp_excluded: Vec<(String, String)>,
    pub dhcp_pools: IndexMap<String, Pool>,
    pub acls: IndexMap<String, Acl>,
    pub nat: Nat,
    pub ssh: Ssh,
    pub crypto_key: Option<i64>,
    pub users: IndexMap<String, User>,
    pub lines: Lines,
    pub logging: Logging,
    pub ntp_servers: Vec<String>,
    pub ntp_source: Option<String>,
    pub timestamps_msec: bool,
    pub ip_routing: bool,
    pub crypto: Option<Crypto>,
    pub ospf: Option<Ospf>,
    pub bgp: Option<Bgp>,
    pub x: IndexMap<String, String>,
    pub banners: IndexMap<String, Banner>,
    pub min_pw_len: i64,
    pub aliases: IndexMap<String, String>,
    pub conf_reg: Option<String>,
    pub conf_reg_next: Option<String>,
    pub err_recovery: bool,
    pub cdp_off: bool,
    pub lldp: bool,
    pub vtp: Option<Vtp>,
    pub snmp: Option<Snmp>,
    pub pw_encrypt: bool,
    pub scp_server: bool,
    pub ntp_prefer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Counters {
    pub inp: f64,
    pub outp: f64,
    pub crc: f64,
    pub late: f64,
    pub runts: f64,
    pub coll: f64,
    pub flaps: f64,
    pub cleared: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub mac: Option<String>,
    pub pool: String,
    pub conflict: bool,
    pub t: f64,
    pub host: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NatTrans {
    pub proto: String,
    pub global: String,
    pub local: String,
    pub port: i64,
    pub dst: String,
    pub dport: i64,
    pub from_static: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ike {
    pub since: f64,
    pub conn_id: u32,
    pub up: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaCount {
    pub encaps: f64,
    pub decaps: f64,
    pub since: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClockSet {
    pub ms: f64,
    pub at: f64,
}

/// Körtidsdata som inte är konfiguration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Rt {
    pub logs: Vec<String>,
    pub dhcp_bindings: IndexMap<String, Binding>,
    pub nat_trans: Vec<NatTrans>,
    pub acl_counters: IndexMap<String, BTreeMap<i64, f64>>,
    pub errdisabled: IndexMap<String, String>,
    pub sec_learned: IndexMap<String, Vec<String>>,
    pub sec_violations: i64,
    pub counters: IndexMap<String, Counters>,
    pub boot_time: f64,
    pub reloads: i64,
    pub ike: IndexMap<String, Ike>,
    pub ipsec: IndexMap<String, SaCount>,
    pub p2fail: IndexMap<String, bool>,
    pub pinged: IndexMap<String, bool>,
    pub clock_set: Option<ClockSet>,
    pub debug: IndexMap<String, bool>,
    pub reload_at: Option<f64>,
    pub vlan_dat_deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StaticIp {
    pub ip: String,
    pub mask: String,
    pub gw: Option<String>,
    pub dns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nic {
    pub mac: String,
    pub dhcp: bool,
    pub static_: Option<StaticIp>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Lease {
    pub ip: String,
    pub mask: String,
    pub gw: Option<String>,
    pub dns: Vec<String>,
    pub server: Option<String>,
    pub pool: Option<String>,
    pub obtained: f64,
    pub apipa: bool,
    pub nolink: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArpEntry {
    pub mac: String,
    pub t: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WlcIface {
    pub vlan: u16,
    pub ip: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wlan {
    pub ssid: String,
    pub iface: String,
    pub enabled: bool,
    pub security: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WlcSaved {
    pub wlans: BTreeMap<u32, Wlan>,
    pub ap_channels: IndexMap<String, u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wlc {
    pub sys_name: String,
    pub interfaces: IndexMap<String, WlcIface>,
    pub wlans: BTreeMap<u32, Wlan>,
    pub ap_channels: IndexMap<String, u32>,
    pub saved: Option<WlcSaved>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMember {
    pub name: String,
    pub ip: String,
    pub port: i64,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbPool {
    pub vip: String,
    pub port: i64,
    pub method: String,
    pub monitor: String,
    pub members: Vec<LbMember>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct LbStat {
    pub req: i64,
    pub fail: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lb {
    pub name: String,
    pub pools: IndexMap<String, LbPool>,
    pub rr: usize,
    pub stats: IndexMap<String, LbStat>,
    pub saved: Option<IndexMap<String, LbPool>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IspAdv {
    pub net: String,
    pub mask: String,
    pub path: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IspBgp {
    pub as_: i64,
    pub neighbors: IndexMap<String, BgpNeighbor>,
    pub advertise: Vec<IspAdv>,
}

/// En enhet i nätet. Vilka delar som finns beror på typen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub kind: Kind,
    pub os: Os,
    pub model: String,
    pub site: String,
    pub powered: bool,
    pub label: String,
    pub user: Option<String>,
    pub base_mac: String,
    pub config: Option<IosConfig>,
    pub startup: Option<IosConfig>,
    pub rt: Rt,
    pub poe_budget: f64,
    pub hwic: bool,
    pub nic: Option<Nic>,
    pub lease: Option<Lease>,
    pub arp: IndexMap<String, ArpEntry>,
    pub services: Vec<String>,
    pub poe_class: i64,
    pub poe_watts: f64,
    pub external_power: bool,
    pub wireless: bool,
    pub ssid: Option<String>,
    // Operatörer
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub bgp: Option<IspBgp>,
    pub wlc: Option<Wlc>,
    pub lb: Option<Lb>,
}

impl Device {
    pub fn is_ios(&self) -> bool {
        self.os == Os::Ios
    }
    pub fn cfg(&self) -> &IosConfig {
        self.config.as_ref().expect("IOS-enhet")
    }
    pub fn cfg_mut(&mut self) -> &mut IosConfig {
        self.config.as_mut().expect("IOS-enhet")
    }
    pub fn ifc(&self, name: &str) -> Option<&Iface> {
        self.config.as_ref().and_then(|c| c.ifaces.get(name))
    }
    pub fn nic(&self) -> &Nic {
        self.nic.as_ref().expect("nic")
    }
    pub fn mac(&self) -> String {
        self.nic.as_ref().map(|n| n.mac.clone()).unwrap_or_else(|| self.base_mac.clone())
    }
    fn blank(id: &str, kind: Kind, os: Os, site: &str) -> Device {
        Device {
            id: id.into(),
            kind,
            os,
            model: String::new(),
            site: site.into(),
            powered: true,
            label: id.into(),
            user: None,
            base_mac: mac(id, 0),
            config: None,
            startup: None,
            rt: Rt::default(),
            poe_budget: 0.0,
            hwic: false,
            nic: None,
            lease: None,
            arp: IndexMap::new(),
            services: vec![],
            poe_class: 0,
            poe_watts: 0.0,
            external_power: false,
            wireless: false,
            ssid: None,
            ip: None,
            mask: None,
            bgp: None,
            wlc: None,
            lb: None,
        }
    }
    pub fn simple(id: &str, kind: Kind, site: &str, label: &str) -> Device {
        let mut d = Device::blank(id, kind, Os::None, site);
        d.label = label.into();
        d
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct End {
    pub dev: String,
    pub port: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    Copper,
    Fiber,
    Wan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
    Ok,
    Broken,
    Flapping,
    Unplugged,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub id: String,
    pub a: End,
    pub b: End,
    pub kind: LinkKind,
    pub state: LinkState,
    pub color: String,
    pub serial: bool,
}

impl Link {
    pub fn new(id: &str, a: &str, ap: &str, b: &str, bp: &str, kind: LinkKind, color: &str) -> Link {
        Link {
            id: id.into(),
            a: End { dev: a.into(), port: ap.into() },
            b: End { dev: b.into(), port: bp.into() },
            kind,
            state: LinkState::Ok,
            color: color.into(),
            serial: false,
        }
    }
}

pub fn mac(id: &str, n: u32) -> String {
    let h = format!("{:08x}", U::hash(&format!("{}:{}", id, n)));
    const OUI: [&str; 8] = ["3cd92b", "a4c3f0", "001aa1", "0011bc", "5c5015", "f8b156", "84b802", "00227e"];
    let oui = OUI[(U::hash(id) % 8) as usize];
    format!("{}{}", oui, h)[..12].to_string()
}

fn base_config(kind: Kind) -> IosConfig {
    IosConfig {
        hostname: if kind == Kind::Router { "Router" } else { "Switch" }.into(),
        enable_secret: None,
        enable_plain: None,
        enable_password: None,
        domain_name: None,
        domain_lookup: true,
        name_servers: vec![],
        dns_server: false,
        hosts: IndexMap::new(),
        ifaces: IndexMap::new(),
        vlans: BTreeMap::new(),
        vlan_state: BTreeMap::new(),
        stp_off: vec![],
        stp_priority: BTreeMap::new(),
        routes: vec![],
        default_gateway: None,
        dhcp_excluded: vec![],
        dhcp_pools: IndexMap::new(),
        acls: IndexMap::new(),
        nat: Nat::default(),
        ssh: Ssh::default(),
        crypto_key: None,
        users: IndexMap::new(),
        lines: Lines {
            con: Line { speed: 9600, ..Default::default() },
            vty0_4: Lines::vty("all", false),
            vty5_15: Lines::vty("all", false),
        },
        logging: Logging { buffered: true, size: 4096, hosts: vec![], level: None },
        ntp_servers: vec![],
        ntp_source: None,
        timestamps_msec: false,
        ip_routing: kind == Kind::Router,
        crypto: None,
        ospf: None,
        bgp: None,
        x: IndexMap::new(),
        banners: IndexMap::new(),
        min_pw_len: 0,
        aliases: IndexMap::new(),
        conf_reg: None,
        conf_reg_next: None,
        err_recovery: false,
        cdp_off: false,
        lldp: false,
        vtp: None,
        snmp: None,
        pw_encrypt: false,
        scp_server: false,
        ntp_prefer: None,
    }
}

fn base_ios(id: &str, model: &str, kind: Kind, site: &str) -> Device {
    let mut d = Device::blank(id, kind, Os::Ios, site);
    d.model = model.into();
    d.config = Some(base_config(kind));
    d.rt.boot_time = -86400.0 * 12.0 - 3600.0 * 5.0;
    d
}

pub fn make_switch(id: &str, model: &str, site: &str) -> Device {
    let mut d = base_ios(id, model, Kind::Switch, site);
    let c = d.cfg_mut();
    for i in 1..=28 {
        c.ifaces.insert(format!("GigabitEthernet0/{}", i), Iface::switch_port(i));
    }
    let mut v1 = Iface::svi(1);
    v1.shutdown = true;
    c.ifaces.insert("Vlan1".into(), v1);
    c.vlans.insert(1, "default".into());
    d.poe_budget = if model.ends_with("PS") { 370.0 } else { 0.0 };
    d
}

pub fn make_router(id: &str, model: &str, site: &str, serial: bool) -> Device {
    let mut d = base_ios(id, model, Kind::Router, site);
    let c = d.cfg_mut();
    for n in ["GigabitEthernet0/0", "GigabitEthernet0/1", "GigabitEthernet0/2"] {
        c.ifaces.insert(n.into(), Iface::router_port());
    }
    if serial {
        for n in ["Serial0/0/0", "Serial0/0/1"] {
            c.ifaces.insert(n.into(), Iface::router_port());
        }
    }
    c.ifaces.insert(
        "Embedded-Service-Engine0/0".into(),
        Iface { shutdown: true, internal: true, ..Default::default() },
    );
    d.hwic = serial;
    d
}

pub struct HostOpts<'a> {
    pub kind: Kind,
    pub site: &'a str,
    pub label: &'a str,
    pub user: Option<&'a str>,
    pub dhcp: bool,
    pub ip: Option<(&'a str, &'a str, Option<&'a str>, Vec<&'a str>)>,
    pub services: Vec<&'a str>,
    pub poe: Option<(i64, f64)>,
    pub wireless: bool,
}

impl<'a> HostOpts<'a> {
    pub fn new(label: &'a str) -> Self {
        HostOpts { kind: Kind::Host, site: "gbg", label, user: None, dhcp: false, ip: None, services: vec![], poe: None, wireless: false }
    }
}

pub fn make_host(id: &str, os: Os, o: HostOpts) -> Device {
    let mut d = Device::blank(id, o.kind, os, o.site);
    d.label = o.label.into();
    d.user = o.user.map(|s| s.to_string());
    d.nic = Some(Nic {
        mac: mac(id, 7),
        dhcp: o.dhcp,
        static_: o.ip.map(|(ip, mask, gw, dns)| StaticIp {
            ip: ip.into(),
            mask: mask.into(),
            gw: gw.map(|g| g.to_string()),
            dns: dns.into_iter().map(|s| s.to_string()).collect(),
        }),
        enabled: true,
    });
    d.services = o.services.into_iter().map(|s| s.to_string()).collect();
    if let Some((c, w)) = o.poe {
        d.poe_class = c;
        d.poe_watts = w;
    }
    d.wireless = o.wireless;
    d
}

fn sub(d: &mut Device, name: &str, vlan: u16, ip: &str, mask: &str, nat: Option<NatDir>, acl_in: Option<&str>, desc: &str) {
    let parent = name.split('.').next().unwrap().to_string();
    let mut i = Iface::sub(&parent);
    i.dot1q = Some(vlan);
    i.ip = Some(IpAddr { addr: ip.into(), mask: mask.into() });
    i.nat_dir = nat;
    i.acl_in = acl_in.map(|s| s.to_string());
    i.description = desc.into();
    d.cfg_mut().ifaces.insert(name.into(), i);
}

fn gi(d: &mut Device, port: u32) -> &mut Iface {
    d.cfg_mut().ifaces.get_mut(&format!("GigabitEthernet0/{}", port)).unwrap()
}

fn access<'a>(d: &'a mut Device, port: u32, vlan: u16, desc: &str) -> &'a mut Iface {
    let p = gi(d, port);
    p.mode = "access".into();
    p.access_vlan = vlan;
    p.description = desc.into();
    p.portfast = true;
    p
}

fn trunk<'a>(d: &'a mut Device, port: u32, desc: &str, allowed: &[u16], native: u16) -> &'a mut Iface {
    let p = gi(d, port);
    p.mode = "trunk".into();
    p.encap = Some("dot1q".into());
    p.allowed = Some(VlanList::List(allowed.to_vec()));
    p.native = native;
    p.description = desc.into();
    p.nonegotiate = true;
    p
}

fn common_mgmt(d: &mut Device) {
    let c = d.cfg_mut();
    // Inget enable-lösenord: konsolen går rakt in. Bara SSH kräver ett konto (drift / nordvik, står på lappen).
    c.enable_secret = None;
    c.enable_plain = None;
    c.domain_name = Some("nordvik.example".into());
    c.users.insert(
        "drift".into(),
        User {
            priv_: 15,
            secret: "$9$Kx7mP2qRwT$e8Ld0Qf3vB1nZ6yHs5gJ4aUcW9oXrEiM2tNpA7kYl".into(),
            plain: "nordvik".into(),
            by_game: true,
            pw_type: None,
        },
    );
    c.ssh.version = Some(2);
    c.crypto_key = Some(2048);
    c.lines.con.log_sync = true;
    c.lines.vty0_4 = Lines::vty("ssh", true);
    c.lines.vty5_15 = Lines::vty("ssh", true);
    c.logging = Logging { buffered: true, size: 16384, hosts: vec!["192.168.1.17".into()], level: None };
    c.ntp_servers = vec!["192.168.1.16".into()];
    c.timestamps_msec = true;
}

fn r(net: &str, mask: &str, nh: &str) -> Route {
    Route { net: net.into(), mask: mask.into(), nh: Some(nh.into()), iface: None, ad: None }
}

fn pool(net: &str, mask: &str, gw: &str, dns: &str) -> Pool {
    Pool {
        network: Some(net.into()),
        mask: Some(mask.into()),
        default_router: vec![gw.into()],
        dns: vec![dns.into()],
        lease: Some(1.0),
        opt150: None,
    }
}

fn ipa(a: &str, m: &str) -> Option<IpAddr> {
    Some(IpAddr { addr: a.into(), mask: m.into() })
}

/// Hela nätet: enheter, länkar och tid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Topology {
    pub devices: IndexMap<String, Device>,
    pub links: Vec<Link>,
}

pub fn build_golden(week: u32) -> Topology {
    // Vecka 10 och kursens vecka 12 (02B) går Borås över internet i en IPsec-tunnel.
    // Kurs 2 (vecka 11–14, Routing och switching) lägger till Mölndal och operatörerna.
    let w = week;
    let vpn = w == 10 || w == 12;
    let k2 = w >= 11;
    let mut devices: IndexMap<String, Device> = IndexMap::new();
    let mut links: Vec<Link> = vec![];
    macro_rules! link {
        ($a:expr, $ap:expr, $b:expr, $bp:expr, $kind:expr, $color:expr) => {{
            let id = format!("L{}", links.len());
            links.push(Link::new(&id, $a, $ap, $b, $bp, $kind, $color));
        }};
    }
    use LinkKind::*;

    // ---------------- Göteborg ----------------
    let mut r1 = make_router("R1", "CISCO2951/K9", "gbg", k2);
    common_mgmt(&mut r1);
    {
        let c = r1.cfg_mut();
        c.hostname = "R-Nordvik-1".into();
        c.name_servers = vec!["9.9.9.9".into()];
        c.dns_server = true;
        for (k, v) in [
            ("filserver", "192.168.1.10"),
            ("skrivare", "192.168.1.11"),
            ("ntp", "192.168.1.16"),
            ("logg", "192.168.1.17"),
            ("ekonomi", "192.168.1.70"),
            ("lager-pc", "192.168.2.20"),
        ] {
            c.hosts.insert(k.into(), v.into());
        }
        let g0 = c.ifaces.get_mut("GigabitEthernet0/0").unwrap();
        g0.shutdown = false;
        g0.description = "Trunk mot SW-Nordvik-1".into();
    }
    sub(&mut r1, "GigabitEthernet0/0.10", 10, "192.168.1.1", "255.255.255.192", Some(NatDir::Inside), None, "KONTOR");
    sub(&mut r1, "GigabitEthernet0/0.20", 20, "192.168.1.65", "255.255.255.192", Some(NatDir::Inside), None, "EKONOMI");
    sub(&mut r1, "GigabitEthernet0/0.30", 30, "192.168.1.129", "255.255.255.192", Some(NatDir::Inside), Some("GAST"), "GAST");
    sub(&mut r1, "GigabitEthernet0/0.99", 99, "192.168.1.193", "255.255.255.192", Some(NatDir::Inside), None, "DRIFT");
    {
        let c = r1.cfg_mut();
        let g1 = c.ifaces.get_mut("GigabitEthernet0/1").unwrap();
        g1.shutdown = false;
        g1.ip = ipa("203.0.113.10", "255.255.255.0");
        g1.nat_dir = Some(NatDir::Outside);
        g1.description = "Ut mot operatoren".into();
        let g2 = c.ifaces.get_mut("GigabitEthernet0/2").unwrap();
        g2.shutdown = false;
        g2.ip = ipa("10.0.0.1", "255.255.255.252");
        g2.nat_dir = Some(NatDir::Inside);
        g2.description = "Lank mot Boras".into();
        c.routes = vec![r("0.0.0.0", "0.0.0.0", "203.0.113.1"), r("192.168.2.0", "255.255.255.0", "10.0.0.2")];
        c.dhcp_excluded = vec![
            ("192.168.1.1".into(), "192.168.1.19".into()),
            ("192.168.1.65".into(), "192.168.1.79".into()),
            ("192.168.1.129".into(), "192.168.1.139".into()),
        ];
        c.dhcp_pools.insert("KONTOR".into(), pool("192.168.1.0", "255.255.255.192", "192.168.1.1", "192.168.1.1"));
        c.dhcp_pools.insert("EKONOMI".into(), pool("192.168.1.64", "255.255.255.192", "192.168.1.65", "192.168.1.65"));
        c.dhcp_pools.insert("GAST".into(), pool("192.168.1.128", "255.255.255.192", "192.168.1.129", "9.9.9.9"));
        c.acls.insert(
            "1".into(),
            Acl {
                extended: false,
                rules: vec![
                    Ace::std(10, "permit", AddrSpec::net("192.168.1.0", "0.0.0.255")),
                    Ace::std(20, "permit", AddrSpec::net("192.168.2.0", "0.0.0.255")),
                ],
            },
        );
        c.acls.insert(
            "GAST".into(),
            Acl {
                extended: true,
                rules: vec![
                    Ace::ext(10, "deny", AddrSpec::net("192.168.1.128", "0.0.0.63"), AddrSpec::net("192.168.1.0", "0.0.0.255")),
                    Ace::ext(20, "deny", AddrSpec::net("192.168.1.128", "0.0.0.63"), AddrSpec::net("192.168.2.0", "0.0.0.255")),
                    Ace::ext(30, "permit", AddrSpec::any(), AddrSpec::any()),
                ],
            },
        );
        if w >= 9 {
            // Kapitel 9: kontoret ska inte nå ekonominätet
            c.acls.insert(
                "KONTOR-UT".into(),
                Acl {
                    extended: true,
                    rules: vec![
                        Ace::ext(10, "deny", AddrSpec::net("192.168.1.0", "0.0.0.63"), AddrSpec::net("192.168.1.64", "0.0.0.63")),
                        Ace::ext(20, "permit", AddrSpec::any(), AddrSpec::any()),
                    ],
                },
            );
            c.ifaces.get_mut("GigabitEthernet0/0.10").unwrap().acl_in = Some("KONTOR-UT".into());
        }
        c.nat = Nat {
            statics: vec![NatStatic { local: "192.168.1.10".into(), global: "203.0.113.11".into() }],
            dynamic: vec![NatDyn { acl: "1".into(), iface: "GigabitEthernet0/1".into(), overload: true }],
        };
        if w >= 10 {
            c.hosts.insert("tid".into(), "192.168.1.13".into());
        }
        if vpn {
            // Kapitel 10: den hyrda linan är uppsagd. Borås når Göteborg med IPsec över internet.
            let g2 = c.ifaces.get_mut("GigabitEthernet0/2").unwrap();
            g2.shutdown = true;
            g2.ip = None;
            g2.nat_dir = None;
            g2.description = "Hyrd lina uppsagd".into();
            c.routes = vec![r("0.0.0.0", "0.0.0.0", "203.0.113.1")];
            c.acls.shift_remove("1");
            let mut nat_ut = Acl {
                extended: true,
                rules: vec![
                    Ace::ext(10, "deny", AddrSpec::net("192.168.1.0", "0.0.0.63"), AddrSpec::net("192.168.2.0", "0.0.0.63")),
                    Ace::ext(20, "deny", AddrSpec::net("192.168.1.192", "0.0.0.63"), AddrSpec::net("192.168.2.192", "0.0.0.63")),
                    Ace::ext(30, "permit", AddrSpec::net("192.168.1.0", "0.0.0.255"), AddrSpec::any()),
                ],
            };
            let vpn_acl = Acl {
                extended: true,
                rules: vec![
                    Ace::ext(10, "permit", AddrSpec::net("192.168.1.0", "0.0.0.63"), AddrSpec::net("192.168.2.0", "0.0.0.63")),
                    Ace::ext(20, "permit", AddrSpec::net("192.168.1.192", "0.0.0.63"), AddrSpec::net("192.168.2.192", "0.0.0.63")),
                ],
            };
            c.nat.dynamic = vec![NatDyn { acl: "NAT-UT".into(), iface: "GigabitEthernet0/1".into(), overload: true }];
            c.crypto = Some(vpn_crypto("203.0.113.20", w == 12));
            let g1 = c.ifaces.get_mut("GigabitEthernet0/1").unwrap();
            g1.crypto_map = Some(if w == 12 { "CMAP" } else { "VPN-MAP" }.into());
            g1.adjust_mss = Some(1360);
            if w == 12 {
                // Kursens 02B: samma tunnel med labbens namn (VPN-BORAS, TS-NORDVIK, CMAP)
                nat_ut.rules.push(Ace::ext(40, "permit", AddrSpec::net("192.168.3.0", "0.0.0.63"), AddrSpec::any()));
                c.acls.insert("NAT-UT".into(), nat_ut);
                c.acls.insert("VPN-BORAS".into(), vpn_acl);
            } else {
                c.acls.insert("NAT-UT".into(), nat_ut);
                c.acls.insert("VPN-TRAFIK".into(), vpn_acl);
            }
        }
    }
    devices.insert("R1".into(), r1);

    let mut sw1 = make_switch("SW1", "WS-C3560G-24PS", "gbg");
    common_mgmt(&mut sw1);
    {
        let c = sw1.cfg_mut();
        c.hostname = "SW-Nordvik-1".into();
        c.vlans = [(1, "default"), (10, "KONTOR"), (20, "EKONOMI"), (30, "GAST"), (99, "DRIFT")]
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
        c.stp_priority = [(10, 24576), (20, 24576), (30, 24576), (99, 24576)].into_iter().collect();
    }
    trunk(&mut sw1, 1, "Trunk mot R-Nordvik-1", &[10, 20, 30, 99], 999);
    access(&mut sw1, 5, 10, "Anna");
    access(&mut sw1, 6, 10, "Karim");
    access(&mut sw1, 7, 20, "Bo");
    access(&mut sw1, 8, 20, "Maja");
    access(&mut sw1, 9, 30, "Reception gast").port_sec =
        Some(PortSec { max: 1, violation: "shutdown".into(), sticky: true, macs: vec![], disabled: false });
    access(&mut sw1, 10, 99, "Teknikerbanken");
    access(&mut sw1, 11, 10, "Skrivare");
    access(&mut sw1, 12, 10, "Sara");
    access(&mut sw1, 20, 99, "WLC-Nordvik");
    trunk(&mut sw1, 24, "Trunk mot SW-Nordvik-2", &[10, 20, 30, 99], 999);
    trunk(&mut sw1, 25, "Reservlank mot SW-Nordvik-2", &[10, 20, 30, 99], 999);
    {
        let c = sw1.cfg_mut();
        let mut v = Iface::svi(99);
        v.ip = ipa("192.168.1.194", "255.255.255.192");
        c.ifaces.insert("Vlan99".into(), v);
        c.default_gateway = Some("192.168.1.193".into());
    }
    devices.insert("SW1".into(), sw1);

    let mut sw2 = make_switch("SW2", "WS-C3750G-24PS", "gbg");
    common_mgmt(&mut sw2);
    {
        let c = sw2.cfg_mut();
        c.hostname = "SW-Nordvik-2".into();
        c.vlans = [(1, "default"), (10, "KONTOR"), (20, "EKONOMI"), (30, "GAST"), (99, "DRIFT")]
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
    }
    access(&mut sw2, 2, 10, "Filserver");
    access(&mut sw2, 3, 10, "NTP-server");
    access(&mut sw2, 4, 10, "Loggserver");
    access(&mut sw2, 5, 10, "Lisa");
    access(&mut sw2, 8, 20, "Ekonomisystem");
    trunk(&mut sw2, 24, "Trunk mot SW-Nordvik-1", &[10, 20, 30, 99], 999);
    trunk(&mut sw2, 25, "Reservlank mot SW-Nordvik-1", &[10, 20, 30, 99], 999);
    {
        let c = sw2.cfg_mut();
        let mut v = Iface::svi(99);
        v.ip = ipa("192.168.1.195", "255.255.255.192");
        c.ifaces.insert("Vlan99".into(), v);
        c.default_gateway = Some("192.168.1.193".into());
    }
    devices.insert("SW2".into(), sw2);

    let mut add = |d: Device| {
        devices.insert(d.id.clone(), d);
    };
    // Värdar i Göteborg
    for (id, label, user) in [
        ("PC-Anna", "Annas dator", "Anna"),
        ("PC-Karim", "Karims dator", "Karim"),
        ("PC-Bo", "Bos dator", "Bo"),
        ("PC-Maja", "Majas dator", "Maja"),
        ("PC-Sara", "Saras dator", "Sara"),
        ("PC-Lisa", "Lisas dator", "Lisa"),
        ("Gast-laptop", "Gästlaptop i receptionen", "Gäst"),
    ] {
        let mut o = HostOpts::new(label);
        o.dhcp = true;
        o.user = Some(user);
        add(make_host(id, Os::Windows, o));
    }
    let mut o = HostOpts::new("Din laptop");
    o.ip = Some(("192.168.1.200", "255.255.255.192", Some("192.168.1.193"), vec!["192.168.1.193"]));
    add(make_host("Tekniker", Os::Linux, o));
    let mut o = HostOpts::new("Skrivare");
    o.kind = Kind::Printer;
    o.ip = Some(("192.168.1.11", "255.255.255.192", Some("192.168.1.1"), vec![]));
    add(make_host("Skrivare", Os::Device, o));
    for (id, ip, gw, dns, label, svc) in [
        ("Filserver", "192.168.1.10", "192.168.1.1", Some("192.168.1.1"), "Filserver", "smb"),
        ("NTP-server", "192.168.1.16", "192.168.1.1", None, "NTP-server", "ntp"),
        ("Loggserver", "192.168.1.17", "192.168.1.1", None, "Loggserver", "syslog"),
        ("Ekonomisystem", "192.168.1.70", "192.168.1.65", None, "Ekonomisystem", "https"),
    ] {
        let mut o = HostOpts::new(label);
        o.kind = Kind::Server;
        o.ip = Some((ip, "255.255.255.192", Some(gw), dns.into_iter().collect()));
        o.services = vec![svc];
        add(make_host(id, Os::Server, o));
    }
    let mut o = HostOpts::new("WLC-Nordvik (AIR-WLC4402)");
    o.kind = Kind::Wlc;
    o.ip = Some(("192.168.1.196", "255.255.255.192", Some("192.168.1.193"), vec![]));
    add(make_host("WLC", Os::Wlc, o));
    // Oanvänd brandvägg i racket (kapitel 9 använder routerns ACL:er)
    add(Device::simple("ASA", Kind::Asa, "gbg", "ASA5540 (inte i drift än)"));

    let gast_mac = devices["Gast-laptop"].nic().mac.clone();
    devices
        .get_mut("SW1")
        .unwrap()
        .cfg_mut()
        .ifaces
        .get_mut("GigabitEthernet0/9")
        .unwrap()
        .port_sec
        .as_mut()
        .unwrap()
        .macs = vec![gast_mac];

    link!("SW1", "GigabitEthernet0/1", "R1", "GigabitEthernet0/0", Copper, "yellow");
    link!("SW1", "GigabitEthernet0/24", "SW2", "GigabitEthernet0/24", Copper, "yellow");
    link!("SW1", "GigabitEthernet0/25", "SW2", "GigabitEthernet0/25", Fiber, "orange");
    link!("SW1", "GigabitEthernet0/5", "PC-Anna", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/6", "PC-Karim", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/7", "PC-Bo", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/8", "PC-Maja", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/9", "Gast-laptop", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/10", "Tekniker", "nic", Copper, "green");
    link!("SW1", "GigabitEthernet0/11", "Skrivare", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/12", "PC-Sara", "nic", Copper, "blue");
    link!("SW1", "GigabitEthernet0/20", "WLC", "nic", Copper, "green");
    link!("SW2", "GigabitEthernet0/2", "Filserver", "nic", Copper, "gray");
    link!("SW2", "GigabitEthernet0/3", "NTP-server", "nic", Copper, "gray");
    link!("SW2", "GigabitEthernet0/4", "Loggserver", "nic", Copper, "gray");
    link!("SW2", "GigabitEthernet0/5", "PC-Lisa", "nic", Copper, "blue");
    link!("SW2", "GigabitEthernet0/8", "Ekonomisystem", "nic", Copper, "gray");

    if w >= 10 {
        // Tidrapporteringen: två webbservrar bakom en lastbalanserare
        for (id, os, ip, label, kind) in [
            ("LB", Os::Lb, "192.168.1.13", "LB-Nordvik (lastbalanserare)", Kind::Lb),
            ("Tid-1", Os::Server, "192.168.1.14", "Tidrapport 1", Kind::Server),
            ("Tid-2", Os::Server, "192.168.1.15", "Tidrapport 2", Kind::Server),
        ] {
            let mut o = HostOpts::new(label);
            o.kind = kind;
            o.ip = Some((ip, "255.255.255.192", Some("192.168.1.1"), vec![]));
            o.services = vec!["http"];
            devices.insert(id.into(), make_host(id, os, o));
        }
        let sw2 = devices.get_mut("SW2").unwrap();
        access(sw2, 6, 10, "LB-Nordvik");
        access(sw2, 9, 10, "Tid-1");
        access(sw2, 10, 10, "Tid-2");
        link!("SW2", "GigabitEthernet0/6", "LB", "nic", Copper, "green");
        link!("SW2", "GigabitEthernet0/9", "Tid-1", "nic", Copper, "gray");
        link!("SW2", "GigabitEthernet0/10", "Tid-2", "nic", Copper, "gray");
        let mut pools = IndexMap::new();
        pools.insert(
            "TID-POOL".into(),
            LbPool {
                vip: "192.168.1.13".into(),
                port: 80,
                method: "round-robin".into(),
                monitor: "http".into(),
                members: vec![
                    LbMember { name: "tid-1".into(), ip: "192.168.1.14".into(), port: 80, enabled: true },
                    LbMember { name: "tid-2".into(), ip: "192.168.1.15".into(), port: 80, enabled: true },
                ],
            },
        );
        devices.get_mut("LB").unwrap().lb =
            Some(Lb { name: "LB-Nordvik".into(), pools, rr: 0, stats: IndexMap::new(), saved: None });
    }

    // ---------------- Operatören / internet ----------------
    let mut isp = Device::simple("ISP", Kind::Isp, "internet", "ISP");
    isp.ip = Some("203.0.113.1".into());
    isp.mask = Some("255.255.255.0".into());
    devices.insert("ISP".into(), isp);
    link!("R1", "GigabitEthernet0/1", "ISP", "nic", Fiber, "red");

    // ---------------- Borås ----------------
    let mut rb = make_router("RB", "CISCO2951/K9", "boras", k2 && !vpn);
    common_mgmt(&mut rb);
    {
        let c = rb.cfg_mut();
        c.hostname = "R-Boras-1".into();
        c.name_servers = vec!["9.9.9.9".into()];
        c.dns_server = true;
        c.hosts.insert("filserver".into(), "192.168.1.10".into());
        c.hosts.insert("ekonomi".into(), "192.168.1.70".into());
        let g0 = c.ifaces.get_mut("GigabitEthernet0/0").unwrap();
        g0.shutdown = false;
        g0.description = "Trunk mot SW-Boras-1".into();
    }
    sub(&mut rb, "GigabitEthernet0/0.40", 40, "192.168.2.1", "255.255.255.192", None, None, "LAGER");
    sub(&mut rb, "GigabitEthernet0/0.50", 50, "192.168.2.65", "255.255.255.192", None, Some("GAST-B"), "TRADLOST-GAST");
    sub(&mut rb, "GigabitEthernet0/0.99", 99, "192.168.2.193", "255.255.255.192", None, None, "DRIFT");
    {
        let c = rb.cfg_mut();
        let b2 = c.ifaces.get_mut("GigabitEthernet0/2").unwrap();
        b2.shutdown = false;
        b2.ip = ipa("10.0.0.2", "255.255.255.252");
        b2.description = "Lank mot Goteborg".into();
        c.routes = vec![r("0.0.0.0", "0.0.0.0", "10.0.0.1")];
        c.dhcp_excluded = vec![
            ("192.168.2.1".into(), "192.168.2.19".into()),
            ("192.168.2.65".into(), "192.168.2.69".into()),
        ];
        c.dhcp_pools.insert("LAGER".into(), pool("192.168.2.0", "255.255.255.192", "192.168.2.1", "192.168.2.1"));
        c.dhcp_pools.insert("TRADLOST-GAST".into(), pool("192.168.2.64", "255.255.255.192", "192.168.2.65", "9.9.9.9"));
        c.acls.insert(
            "GAST-B".into(),
            Acl {
                extended: true,
                rules: vec![
                    Ace::ext(10, "deny", AddrSpec::net("192.168.2.64", "0.0.0.63"), AddrSpec::net("192.168.0.0", "0.0.255.255")),
                    Ace::ext(20, "permit", AddrSpec::any(), AddrSpec::any()),
                ],
            },
        );
        if vpn {
            let b2 = c.ifaces.get_mut("GigabitEthernet0/2").unwrap();
            b2.shutdown = true;
            b2.ip = None;
            b2.description = "Hyrd lina uppsagd".into();
            let b1 = c.ifaces.get_mut("GigabitEthernet0/1").unwrap();
            b1.shutdown = false;
            b1.ip = ipa("203.0.113.20", "255.255.255.0");
            b1.nat_dir = Some(NatDir::Outside);
            b1.description = "Ut mot operatoren".into();
            b1.crypto_map = Some(if w == 12 { "CMAP" } else { "VPN-MAP" }.into());
            b1.adjust_mss = Some(1360);
            for n in ["GigabitEthernet0/0.40", "GigabitEthernet0/0.50", "GigabitEthernet0/0.99"] {
                c.ifaces.get_mut(n).unwrap().nat_dir = Some(NatDir::Inside);
            }
            c.routes = vec![r("0.0.0.0", "0.0.0.0", "203.0.113.1")];
            c.acls.insert(
                "NAT-UT".into(),
                Acl {
                    extended: true,
                    rules: vec![
                        Ace::ext(10, "deny", AddrSpec::net("192.168.2.0", "0.0.0.63"), AddrSpec::net("192.168.1.0", "0.0.0.63")),
                        Ace::ext(20, "deny", AddrSpec::net("192.168.2.192", "0.0.0.63"), AddrSpec::net("192.168.1.192", "0.0.0.63")),
                        Ace::ext(30, "permit", AddrSpec::net("192.168.2.0", "0.0.0.255"), AddrSpec::any()),
                    ],
                },
            );
            let vpn_acl = Acl {
                extended: true,
                rules: vec![
                    Ace::ext(10, "permit", AddrSpec::net("192.168.2.0", "0.0.0.63"), AddrSpec::net("192.168.1.0", "0.0.0.63")),
                    Ace::ext(20, "permit", AddrSpec::net("192.168.2.192", "0.0.0.63"), AddrSpec::net("192.168.1.192", "0.0.0.63")),
                ],
            };
            c.acls.insert(if w == 12 { "VPN-BORAS" } else { "VPN-TRAFIK" }.into(), vpn_acl);
            c.nat = Nat { statics: vec![], dynamic: vec![NatDyn { acl: "NAT-UT".into(), iface: "GigabitEthernet0/1".into(), overload: true }] };
            c.crypto = Some(vpn_crypto("203.0.113.10", w == 12));
            // Egen trafik från routern ska ha driftadressen som källa, annars krypteras den inte
            c.ntp_source = Some("GigabitEthernet0/0.40".into());
        }
        if w >= 10 {
            c.hosts.insert("tid".into(), "192.168.1.13".into());
        }
    }
    devices.insert("RB".into(), rb);

    let mut swb = make_switch("SWB", "WS-C3560G-24PS", "boras");
    common_mgmt(&mut swb);
    {
        let c = swb.cfg_mut();
        c.hostname = "SW-Boras-1".into();
        c.vlans = [(1, "default"), (40, "LAGER"), (50, "TRADLOST-GAST"), (99, "DRIFT")]
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
        c.stp_priority = [(40, 24576), (50, 24576), (99, 24576)].into_iter().collect();
    }
    for n in 1..=3 {
        let p = trunk(&mut swb, n, &format!("AP-Lager-{}", n), &[40, 50, 99], 99);
        p.nonegotiate = true;
        p.portfast = true;
    }
    access(&mut swb, 5, 40, "Lagerdator");
    trunk(&mut swb, 24, "Trunk mot R-Boras-1", &[40, 50, 99], 999);
    {
        let c = swb.cfg_mut();
        let mut v = Iface::svi(99);
        v.ip = ipa("192.168.2.194", "255.255.255.192");
        c.ifaces.insert("Vlan99".into(), v);
        c.default_gateway = Some("192.168.2.193".into());
    }
    devices.insert("SWB".into(), swb);

    let mut o = HostOpts::new("Lagerdatorn");
    o.dhcp = true;
    o.site = "boras";
    o.user = Some("Nils");
    devices.insert("PC-Lager".into(), make_host("PC-Lager", Os::Windows, o));
    for n in 1..=3 {
        let id = format!("AP-Lager-{}", n);
        let label = format!("AP-Lager-{} (AIR-CAP3702)", n);
        let ip = format!("192.168.2.20{}", n);
        let mut o = HostOpts::new(&label);
        o.kind = Kind::Ap;
        o.site = "boras";
        o.ip = Some((&ip, "255.255.255.192", Some("192.168.2.193"), vec![]));
        o.poe = Some((3, 15.4));
        devices.insert(id.clone(), make_host(&id, Os::Ap, o));
    }
    for (id, label, user) in [("Gast-mobil", "Gästens mobil", "Gäst"), ("Lager-skanner", "Streckkodsläsare", "Nils")] {
        let mut o = HostOpts::new(label);
        o.dhcp = true;
        o.site = "boras";
        o.wireless = true;
        o.user = Some(user);
        devices.insert(id.into(), make_host(id, Os::Phone, o));
    }

    link!("SWB", "GigabitEthernet0/24", "RB", "GigabitEthernet0/0", Copper, "yellow");
    link!("SWB", "GigabitEthernet0/1", "AP-Lager-1", "nic", Copper, "white");
    link!("SWB", "GigabitEthernet0/2", "AP-Lager-2", "nic", Copper, "white");
    link!("SWB", "GigabitEthernet0/3", "AP-Lager-3", "nic", Copper, "white");
    link!("SWB", "GigabitEthernet0/5", "PC-Lager", "nic", Copper, "blue");
    if vpn {
        link!("RB", "GigabitEthernet0/1", "ISP", "nic2", Fiber, "red");
    } else {
        link!("R1", "GigabitEthernet0/2", "RB", "GigabitEthernet0/2", Wan, "red");
    }

    if k2 {
        kurs2(w, vpn, &mut devices, &mut links);
    }

    // Trådlösa controllern: AireOS-liknande konfiguration
    let mut ifs = IndexMap::new();
    ifs.insert("management".into(), WlcIface { vlan: 99, ip: "192.168.1.196".into() });
    ifs.insert("lager".into(), WlcIface { vlan: 40, ip: "192.168.2.10".into() });
    ifs.insert("gast".into(), WlcIface { vlan: 50, ip: "192.168.2.66".into() });
    let mut wlans = BTreeMap::new();
    wlans.insert(1, Wlan { ssid: "Nordvik-Lager".into(), iface: "lager".into(), enabled: true, security: "WPA2-PSK".into() });
    wlans.insert(2, Wlan { ssid: "Nordvik-Gast".into(), iface: "gast".into(), enabled: true, security: "WPA2-PSK".into() });
    let mut ch = IndexMap::new();
    ch.insert("AP-Lager-1".into(), 1);
    ch.insert("AP-Lager-2".into(), 6);
    ch.insert("AP-Lager-3".into(), 11);
    devices.get_mut("WLC").unwrap().wlc =
        Some(Wlc { sys_name: "WLC-Nordvik".into(), interfaces: ifs, wlans, ap_channels: ch, saved: None });

    // Startup = running för alla IOS-enheter
    for d in devices.values_mut() {
        if d.is_ios() {
            d.startup = d.config.clone();
        }
        if let Some(w) = &mut d.wlc {
            w.saved = Some(WlcSaved { wlans: w.wlans.clone(), ap_channels: w.ap_channels.clone() });
        }
        if let Some(lb) = &mut d.lb {
            lb.saved = Some(lb.pools.clone());
        }
    }
    Topology { devices, links }
}

/// IPsec site-to-site enligt kapitel 10 (samma nyckel och policy på båda sidor).
/// Kurs 2 (labb 02B) använder labbens värden: DH-grupp 5, SHA och namnen TS-NORDVIK, CMAP och VPN-BORAS.
fn vpn_crypto(peer: &str, kurs: bool) -> Crypto {
    let mut c = Crypto::default();
    c.keys.insert(peer.into(), if kurs { "NordvikLabb123" } else { "Nordvik-VPN-2026" }.into());
    let (hash, group, ts, tr, map, acl) = if kurs {
        ("sha", 5, "TS-NORDVIK", "esp-sha-hmac", "CMAP", "VPN-BORAS")
    } else {
        ("sha256", 14, "NORDVIK-TS", "esp-sha256-hmac", "VPN-MAP", "VPN-TRAFIK")
    };
    c.policies.insert(
        10,
        IsakmpPolicy { enc: "aes 256".into(), hash: hash.into(), auth: "pre-share".into(), group, lifetime: 86400 },
    );
    c.transform_sets.insert(
        ts.into(),
        TransformSet { transforms: vec!["esp-aes".into(), "256".into(), tr.into()], mode: "tunnel".into() },
    );
    let mut m = BTreeMap::new();
    m.insert(10, CmapEntry { peer: Some(peer.into()), ts: Some(ts.into()), acl: Some(acl.into()) });
    c.maps.insert(map.into(), m);
    c
}

fn ospf_conf(rid: &str, networks: Vec<OspfNet>, no_passive: &[&str], default_originate: bool) -> Ospf {
    Ospf {
        pid: 1,
        router_id: Some(rid.into()),
        networks,
        passive_default: true,
        passive: vec![],
        no_passive: no_passive.iter().map(|s| s.to_string()).collect(),
        ref_bw: 100,
        default_originate,
        default_always: false,
        log_adj: true,
    }
}

fn onet(n: &str, w: &str) -> OspfNet {
    OspfNet { net: n.into(), wild: w.into(), area: 0 }
}

// ---------------- Kurs 2: Routing och switching ----------------
// Kursrepots labbar (Packet Tracer 02A, 02B, 03A och 03B): Göteborg är navet, Borås och Mölndal ekrarna.
fn kurs2(w: u32, vpn: bool, devices: &mut IndexMap<String, Device>, links: &mut Vec<Link>) {
    use LinkKind::*;
    macro_rules! link {
        ($a:expr, $ap:expr, $b:expr, $bp:expr, $kind:expr, $serial:expr) => {{
            let id = format!("L{}", links.len());
            let mut l = Link::new(&id, $a, $ap, $b, $bp, $kind, "red");
            l.serial = $serial;
            links.push(l);
        }};
    }
    {
        let c = devices.get_mut("R1").unwrap().cfg_mut();
        let s0 = c.ifaces.get_mut("Serial0/0/0").unwrap();
        s0.shutdown = false;
        s0.ip = ipa("10.0.0.5", "255.255.255.252");
        s0.description = "Hyrd forbindelse mot Molndal".into();
        s0.nat_dir = Some(NatDir::Inside);
        s0.clock_rate = Some(128000);
        c.hosts.insert("molndal-pc".into(), "192.168.3.10".into());
        if !vpn {
            // Ny fiber till Borås: direktlänken är tillbaka, och Borås går ut på internet via Göteborg
            c.ifaces.get_mut("GigabitEthernet0/2").unwrap().description = "Lank mot Boras".into();
            c.acls.get_mut("1").unwrap().rules.push(Ace::std(30, "permit", AddrSpec::net("192.168.3.0", "0.0.0.63")));
        }
        // Navet: en rutt per kontor (vecka 11 och 12). Från vecka 13 sköter OSPF det.
        if w <= 12 {
            c.routes.retain(|r| r.net == "0.0.0.0" || (!vpn && r.net == "192.168.2.0"));
            c.routes.push(r("192.168.3.0", "255.255.255.192", "10.0.0.6"));
        } else {
            c.routes = vec![r("0.0.0.0", "0.0.0.0", "203.0.113.1")];
            c.ospf = Some(ospf_conf(
                "1.1.1.1",
                vec![onet("192.168.1.0", "0.0.0.255"), onet("10.0.0.0", "0.0.0.3"), onet("10.0.0.4", "0.0.0.3")],
                &["GigabitEthernet0/2", "Serial0/0/0"],
                true,
            ));
        }
    }
    if !vpn {
        let rb = devices.get_mut("RB").unwrap().cfg_mut();
        let b1 = rb.ifaces.get_mut("GigabitEthernet0/1").unwrap();
        b1.shutdown = false;
        b1.ip = ipa("10.0.0.9", "255.255.255.252");
        b1.description = "Lank mot Molndal".into();
        if w >= 13 {
            rb.routes = vec![];
            rb.ospf = Some(ospf_conf(
                "2.2.2.2",
                vec![onet("192.168.2.0", "0.0.0.255"), onet("10.0.0.0", "0.0.0.3"), onet("10.0.0.8", "0.0.0.3")],
                &["GigabitEthernet0/2", "GigabitEthernet0/1"],
                false,
            ));
        }
    }

    // R-Molndal-1 och datorn i Mölndal (kontoret som kom med uppköpet av Bergquist)
    let mut rm = make_router("RM", "CISCO2911/K9", "molndal", true);
    common_mgmt(&mut rm);
    {
        let m = rm.cfg_mut();
        m.hostname = "R-Molndal-1".into();
        m.name_servers = vec!["9.9.9.9".into()];
        m.dns_server = true;
        m.hosts.insert("filserver".into(), "192.168.1.10".into());
        m.hosts.insert("ekonomi".into(), "192.168.1.70".into());
        let g0 = m.ifaces.get_mut("GigabitEthernet0/0").unwrap();
        g0.shutdown = false;
        g0.ip = ipa("192.168.3.1", "255.255.255.192");
        g0.description = "LAN Molndal".into();
        let s0 = m.ifaces.get_mut("Serial0/0/0").unwrap();
        s0.shutdown = false;
        s0.ip = ipa("10.0.0.6", "255.255.255.252");
        s0.description = "Hyrd forbindelse mot Goteborg".into();
        if !vpn {
            let g1 = m.ifaces.get_mut("GigabitEthernet0/1").unwrap();
            g1.shutdown = false;
            g1.ip = ipa("10.0.0.10", "255.255.255.252");
            g1.description = "Lank mot Boras".into();
        }
        m.dhcp_excluded = vec![("192.168.3.1".into(), "192.168.3.9".into())];
        m.dhcp_pools.insert("MOLNDAL".into(), pool("192.168.3.0", "255.255.255.192", "192.168.3.1", "192.168.3.1"));
        if w <= 12 {
            m.routes = vec![r("0.0.0.0", "0.0.0.0", "10.0.0.5")];
        } else {
            m.ospf = Some(ospf_conf(
                "3.3.3.3",
                vec![onet("192.168.3.0", "0.0.0.63"), onet("10.0.0.4", "0.0.0.3"), onet("10.0.0.8", "0.0.0.3")],
                &["Serial0/0/0", "GigabitEthernet0/1"],
                false,
            ));
        }
    }
    devices.insert("RM".into(), rm);
    let mut o = HostOpts::new("Elins dator i Mölndal");
    o.dhcp = true;
    o.site = "molndal";
    o.user = Some("Elin");
    devices.insert("PC-Molndal".into(), make_host("PC-Molndal", Os::Windows, o));
    link!("R1", "Serial0/0/0", "RM", "Serial0/0/0", Wan, true);
    if !vpn {
        link!("RB", "GigabitEthernet0/1", "RM", "GigabitEthernet0/1", Wan, false);
    }
    {
        let id = format!("L{}", links.len());
        links.push(Link::new(&id, "RM", "GigabitEthernet0/0", "PC-Molndal", "nic", Copper, "blue"));
    }

    // Vecka 14 (labb 03B): BGP mot ISP-A (AS 64500) och en andra operatör, ISP-B (AS 64501), på Se0/0/1.
    if w >= 14 {
        let c = devices.get_mut("R1").unwrap().cfg_mut();
        let s1 = c.ifaces.get_mut("Serial0/0/1").unwrap();
        s1.shutdown = false;
        s1.ip = ipa("198.51.100.2", "255.255.255.252");
        s1.description = "Ut mot ISP-B".into();
        let mut nb = IndexMap::new();
        nb.insert("203.0.113.1".into(), BgpNeighbor { remote_as: Some(64500), ..Default::default() });
        c.bgp = Some(Bgp { as_: 65010, router_id: Some("1.1.1.1".into()), neighbors: nb, networks: vec![], log_changes: true });
        let isp = devices.get_mut("ISP").unwrap();
        isp.label = "ISP-A".into();
        let mut nb = IndexMap::new();
        nb.insert("203.0.113.10".into(), BgpNeighbor { remote_as: Some(65010), ..Default::default() });
        isp.bgp = Some(IspBgp {
            as_: 64500,
            neighbors: nb,
            advertise: vec![
                IspAdv { net: "203.0.113.0".into(), mask: "255.255.255.0".into(), path: vec![] },
                IspAdv { net: "198.51.100.64".into(), mask: "255.255.255.224".into(), path: vec![64501] },
            ],
        });
        let mut ispb = Device::simple("ISP-B", Kind::Isp, "internet", "ISP-B");
        ispb.ip = Some("198.51.100.1".into());
        ispb.mask = Some("255.255.255.252".into());
        let mut nb = IndexMap::new();
        nb.insert("198.51.100.2".into(), BgpNeighbor { remote_as: Some(65010), ..Default::default() });
        ispb.bgp = Some(IspBgp {
            as_: 64501,
            neighbors: nb,
            advertise: vec![
                IspAdv { net: "198.51.100.64".into(), mask: "255.255.255.224".into(), path: vec![] },
                IspAdv { net: "203.0.113.0".into(), mask: "255.255.255.0".into(), path: vec![64500] },
            ],
        });
        devices.insert("ISP-B".into(), ispb);
        link!("R1", "Serial0/0/1", "ISP-B", "nic", Wan, true);
    }
}

/// Fabriksinställningar för en IOS-enhet (som en ny låda från kartongen)
pub fn factory_config(d: &Device) -> IosConfig {
    let fresh = if d.kind == Kind::Switch {
        make_switch(&d.id, &d.model, &d.site)
    } else {
        make_router(&d.id, &d.model, &d.site, false)
    };
    fresh.config.unwrap()
}
