//! Dynamisk routing för kurs 2: OSPF och eBGP. Port av js/net/dynroute.js.
#![allow(unused_variables, dead_code)]
use super::sim::{Derived, State};
use indexmap::IndexMap;
use std::collections::HashMap;

/// En dynamisk rutt (O, O*E2 eller B)
#[derive(Clone, Debug, Default)]
pub struct DynRoute {
    /// "O", "O*E2" eller "B"
    pub kind: String,
    pub net: String,
    pub mask: String,
    pub nh: String,
    pub iface: String,
    /// "E:dev:iface" för utgående interface
    pub ep_key: String,
    pub metric: i64,
    pub ad: i64,
    pub origin: Option<String>,
    pub path: Vec<i64>,
    /// Avstånd till den som skapar O*E2 (för val mellan flera)
    pub fwd: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct OspfIf {
    pub name: String,
    pub area: i64,
    pub passive: bool,
    pub cost: i64,
    pub hello: i64,
    pub dead: i64,
    pub ip: String,
    pub mask: String,
    pub up: bool,
    pub ep_key: String,
    pub is_loop: bool,
    pub serial: bool,
    pub bw: i64,
}

#[derive(Clone, Debug, Default)]
pub struct OspfNbr {
    pub dev: String,
    pub rid: String,
    pub ip: String,
    pub iface: String,
    pub peer_iface: String,
    pub cost: i64,
    pub dead: i64,
    pub serial: bool,
    /// "DR", "BDR" eller "-"
    pub role: String,
}

#[derive(Clone, Debug, Default)]
pub struct OspfProblem {
    pub iface: String,
    pub peer: String,
    pub peer_ip: String,
    /// "mask", "area", "timers", "rid"
    pub why: String,
    pub peer_area: i64,
    pub peer_hello: i64,
    pub peer_dead: i64,
}

#[derive(Clone, Debug, Default)]
pub struct OspfRouter {
    pub id: String,
    pub rid: Option<String>,
    pub ifs: IndexMap<String, OspfIf>,
    pub nbrs: Vec<OspfNbr>,
    pub problems: Vec<OspfProblem>,
    pub dist: HashMap<String, i64>,
}

#[derive(Clone, Debug, Default)]
pub struct BgpPeer {
    pub ip: String,
    pub remote_as: Option<i64>,
    /// "Active", "Idle", "Idle (Admin)", "Established"
    pub state: String,
    pub up: bool,
    /// "admin", "noas", "noroute", "noreply", "refused", "bad-as", "peer-bad-as"
    pub reason: Option<String>,
    pub pfx: i64,
    pub local_ip: Option<String>,
    pub local_iface: Option<String>,
    pub peer_as: Option<i64>,
    pub peer_dev: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct BgpEntry {
    pub net: String,
    pub mask: String,
    pub nh: String,
    pub path: Vec<i64>,
    pub from: Option<String>,
    pub iface: Option<String>,
    pub ep_key: Option<String>,
    pub local: bool,
    pub best: bool,
}

#[derive(Clone, Debug, Default)]
pub struct BgpInfo {
    pub as_: i64,
    pub rid: Option<String>,
    pub peers: Vec<BgpPeer>,
    pub table: Vec<BgpEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct DynOut {
    pub routes: HashMap<String, Vec<DynRoute>>,
    pub ospf: IndexMap<String, OspfRouter>,
    pub bgp: IndexMap<String, BgpInfo>,
    /// Loggrader som skrivs när beräkningen är klar (enhet, text)
    pub logs: Vec<(String, String)>,
}

/// Administrativt avstånd per rutttyp
pub fn ad(kind: &str) -> i64 {
    match kind {
        "C" | "L" => 0,
        "S" | "S*" => 1,
        "B" => 20,
        _ => 110,
    }
}

/// Räknar BGP och sedan OSPF på D (som redan har eps, comp och by_seg). Sätter d.dynr.
/// quiet = inga loggrader för grannskap.
pub fn compute(st: &mut State, d: &mut Derived, quiet: bool) {
    todo!("dynroute::compute")
}
/// Skriver köade loggrader (d.dynr.logs) med sim::push_log
pub fn flush(st: &mut State, d: &mut Derived) {
    todo!("dynroute::flush")
}
pub fn if_cost(st: &State, dev: &str, iface: &str) -> i64 {
    todo!("dynroute::if_cost")
}
pub fn bandwidth(st: &State, dev: &str, iface: &str) -> i64 {
    todo!("dynroute::bandwidth")
}
pub fn hello(i: Option<&crate::net::model::Iface>) -> i64 {
    i.and_then(|i| i.ospf_hello).unwrap_or(10)
}
pub fn dead(i: Option<&crate::net::model::Iface>) -> i64 {
    i.and_then(|i| i.ospf_dead).unwrap_or_else(|| hello(i) * 4)
}
pub fn router_id(st: &State, d: &Derived, dev: &str) -> Option<String> {
    todo!("dynroute::router_id")
}
/// Sedan när grannskapet/sessionen funnits (dynMem.since)
pub fn since(st: &State, key: &str) -> f64 {
    st.dyn_mem.since.get(key).copied().unwrap_or(0.0)
}
