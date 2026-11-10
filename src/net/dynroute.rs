//! Dynamisk routing för kurs 2: OSPF och eBGP. Port av js/net/dynroute.js.
#![allow(unused_variables, dead_code)]
use super::model::{Kind, Os};
use super::sim::{self, Derived, Ep, State};
use super::util as U;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use super::model::BgpNeighbor;

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

fn is_router(st: &State, id: &str) -> bool {
    st.devices.get(id).map(|d| d.kind == Kind::Router && d.os == Os::Ios).unwrap_or(false)
}

fn eps_of<'a>(d: &'a Derived, dev: &str) -> Vec<&'a Ep> {
    d.eps.values().filter(|e| e.dev == dev).collect()
}

// ------------------------------------------------------------------ OSPF

/// Bandbredd i kbit/s enligt interfacetyp, som IOS standardvärden
fn default_bw(name: &str) -> i64 {
    if name.starts_with("Serial") {
        1544
    } else if name.starts_with("FastEthernet") {
        100000
    } else if name.starts_with("Loopback") {
        8000000
    } else {
        1000000
    }
}

pub fn bandwidth(st: &State, dev: &str, iface: &str) -> i64 {
    let c = st.dev(dev).cfg();
    let i = c.ifaces.get(iface);
    if let Some(b) = i.and_then(|i| i.bandwidth).filter(|b| *b != 0) {
        return b;
    }
    let par = i.and_then(|i| i.parent.as_ref()).and_then(|p| c.ifaces.get(p));
    if let Some(b) = par.and_then(|p| p.bandwidth).filter(|b| *b != 0) {
        return b;
    }
    default_bw(iface)
}

pub fn if_cost(st: &State, dev: &str, iface: &str) -> i64 {
    let c = st.dev(dev).cfg();
    if let Some(oc) = c.ifaces.get(iface).and_then(|i| i.ospf_cost).filter(|c| *c != 0) {
        return oc;
    }
    let r = c.ospf.as_ref().map(|o| o.ref_bw).filter(|r| *r != 0).unwrap_or(100);
    std::cmp::max(1, (r * 1000).div_euclid(bandwidth(st, dev, iface)))
}

fn is_passive(o: &super::model::Ospf, name: &str) -> bool {
    if o.passive_default {
        !o.no_passive.iter().any(|n| n == name)
    } else {
        o.passive.iter().any(|n| n == name)
    }
}

/// Första network-raden som täcker interfacets adress avgör om det är med och i vilken area
fn match_network<'a>(o: &'a super::model::Ospf, ip: &str) -> Option<&'a super::model::OspfNet> {
    o.networks.iter().find(|n| U::wild_match(ip, &n.net, &n.wild))
}

pub fn router_id(st: &State, d: &Derived, dev: &str) -> Option<String> {
    if let Some(rid) = st.dev(dev).config.as_ref().and_then(|c| c.ospf.as_ref()).and_then(|o| o.router_id.clone()).filter(|r| !r.is_empty()) {
        return Some(rid);
    }
    let eps: Vec<&Ep> = eps_of(d, dev).into_iter().filter(|e| e.up && e.ip.is_some()).collect();
    let loops: Vec<&Ep> = eps.iter().copied().filter(|e| e.is_loop).collect();
    let pool = if loops.is_empty() { eps } else { loops };
    let mut best: Option<&Ep> = None;
    for e in pool {
        match best {
            Some(b) if U::ipn(b.ip.as_deref().unwrap()) >= U::ipn(e.ip.as_deref().unwrap()) => {}
            _ => best = Some(e),
        }
    }
    best.and_then(|e| e.ip.clone())
}

fn ospf_ifaces(st: &State, d: &Derived, dev: &str) -> IndexMap<String, OspfIf> {
    let dd = st.dev(dev);
    let c = dd.cfg();
    let o = c.ospf.as_ref().unwrap();
    let mut out = IndexMap::new();
    for (n, i) in c.ifaces.iter() {
        let ip = match &i.ip {
            Some(ip) if !i.internal => ip,
            _ => continue,
        };
        let m = match match_network(o, &ip.addr) {
            Some(m) => m,
            None => continue,
        };
        let ek = format!("E:{}:{}", dev, n);
        let ep = d.eps.get(&ek);
        out.insert(
            n.clone(),
            OspfIf {
                name: n.clone(),
                area: m.area,
                passive: is_passive(o, n),
                cost: if_cost(st, dev, n),
                hello: hello(Some(i)),
                dead: dead(Some(i)),
                ip: ip.addr.clone(),
                mask: ip.mask.clone(),
                up: ep.map(|e| e.up).unwrap_or(false),
                ep_key: ek,
                is_loop: i.is_loop,
                serial: n.starts_with("Serial"),
                bw: bandwidth(st, dev, n),
            },
        );
    }
    out
}

fn ospf(st: &State, d: &Derived, out: &mut DynOut) {
    let mut r: IndexMap<String, OspfRouter> = IndexMap::new();
    for (id, dev) in st.devices.iter() {
        if !is_router(st, id) || dev.cfg().ospf.is_none() {
            continue;
        }
        r.insert(
            id.clone(),
            OspfRouter { id: id.clone(), rid: router_id(st, d, id), ifs: ospf_ifaces(st, d, id), nbrs: vec![], problems: vec![], dist: HashMap::new() },
        );
    }
    // Grannskap: två routrar i samma segment och nät, med samma area, hello och dead, och olika router-id
    let ids: Vec<String> = r.keys().cloned().collect();
    for a in &ids {
        let mut nbrs = vec![];
        let mut problems = vec![];
        {
            let ra = &r[a];
            for (n, ia) in ra.ifs.iter() {
                if !ia.up || ia.passive || ia.is_loop {
                    continue;
                }
                let seg = match d.comp.get(&ia.ep_key) {
                    Some(s) => *s,
                    None => continue,
                };
                for e in d.seg_eps(seg) {
                    if &e.dev == a || !r.contains_key(&e.dev) || !e.up {
                        continue;
                    }
                    let rb = &r[&e.dev];
                    let ib = match rb.ifs.get(&e.iface) {
                        Some(ib) if ib.up && !ib.passive => ib,
                        _ => continue,
                    };
                    let eip = e.ip.clone().unwrap_or_default();
                    if !U::same_subnet(&eip, &ia.ip, &ia.mask) {
                        continue;
                    }
                    let why = if ia.mask != ib.mask {
                        Some("mask")
                    } else if ia.area != ib.area {
                        Some("area")
                    } else if ia.hello != ib.hello || ia.dead != ib.dead {
                        Some("timers")
                    } else if ra.rid == rb.rid {
                        Some("rid")
                    } else {
                        None
                    };
                    if let Some(w) = why {
                        problems.push(OspfProblem { iface: n.clone(), peer: e.dev.clone(), peer_ip: eip, why: w.into(), peer_area: ib.area, peer_hello: ib.hello, peer_dead: ib.dead });
                        continue;
                    }
                    nbrs.push(OspfNbr {
                        dev: e.dev.clone(),
                        rid: rb.rid.clone().unwrap_or_default(),
                        ip: eip,
                        iface: n.clone(),
                        peer_iface: e.iface.clone(),
                        cost: ia.cost,
                        dead: ia.dead,
                        serial: ia.serial,
                        role: String::new(),
                    });
                }
            }
        }
        let ra = r.get_mut(a).unwrap();
        ra.nbrs = nbrs;
        ra.problems = problems;
    }
    // DR och BDR på Ethernet: högst router-id blir DR (alla har prioritet 1)
    for a in &ids {
        let my = U::ipn(r[a].rid.as_deref().unwrap_or(""));
        for nb in r.get_mut(a).unwrap().nbrs.iter_mut() {
            nb.role = if nb.serial {
                "-".into()
            } else if U::ipn(&nb.rid) > my {
                "DR".into()
            } else {
                "BDR".into()
            };
        }
    }
    // Vem har en defaultrutt att sprida (default-information originate)?
    let has_default = |id: &str, out: &DynOut| -> bool {
        let c = st.dev(id).cfg();
        if c.ospf.as_ref().map(|o| o.default_always).unwrap_or(false) {
            return true;
        }
        let stat = c.routes.iter().any(|rt| {
            if rt.net != "0.0.0.0" || rt.mask != "0.0.0.0" {
                return false;
            }
            if let Some(nh) = &rt.nh {
                return eps_of(d, id).iter().any(|e| e.up && e.ip.as_ref().map(|ip| U::same_subnet(nh, ip, e.mask.as_deref().unwrap_or("0.0.0.0"))).unwrap_or(false));
            }
            match &rt.iface {
                Some(i) => d.eps.get(&format!("E:{}:{}", id, i)).map(|e| e.up).unwrap_or(false),
                None => false,
            }
        });
        stat || out.routes.get(id).map(|v| v.iter().any(|x| x.kind == "B" && x.net == "0.0.0.0")).unwrap_or(false)
    };
    // SPF från varje router
    for src in &ids {
        let mut dist: HashMap<String, i64> = HashMap::new();
        let mut first: HashMap<String, OspfNbr> = HashMap::new();
        let mut done: HashSet<String> = HashSet::new();
        dist.insert(src.clone(), 0);
        loop {
            let mut u: Option<String> = None;
            for x in &ids {
                if done.contains(x) {
                    continue;
                }
                if let Some(dx) = dist.get(x) {
                    let better = match &u {
                        None => true,
                        Some(uu) => *dx < dist[uu] || (*dx == dist[uu] && x < uu),
                    };
                    if better {
                        u = Some(x.clone());
                    }
                }
            }
            let u = match u {
                Some(u) => u,
                None => break,
            };
            done.insert(u.clone());
            let nbrs = r[&u].nbrs.clone();
            for nb in nbrs {
                // Grannskapet måste gälla åt båda hållen
                if !r[&nb.dev].nbrs.iter().any(|q| q.dev == u) {
                    continue;
                }
                let nd = dist[&u] + nb.cost;
                let hop = if &u == src { nb.clone() } else { first[&u].clone() };
                let upd = match dist.get(&nb.dev) {
                    None => true,
                    Some(cd) => nd < *cd || (nd == *cd && first.get(&nb.dev).map(|f| U::ipn(&hop.ip) < U::ipn(&f.ip)).unwrap_or(false)),
                };
                if upd {
                    dist.insert(nb.dev.clone(), nd);
                    first.insert(nb.dev.clone(), hop);
                }
            }
        }
        let mut best: IndexMap<String, DynRoute> = IndexMap::new();
        let connected: Vec<String> = eps_of(d, src)
            .iter()
            .filter(|e| e.up && e.ip.is_some())
            .map(|e| format!("{}/{}", U::network(e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("0.0.0.0")), e.mask.clone().unwrap_or_default()))
            .collect();
        for y in &ids {
            if y == src || !dist.contains_key(y) {
                continue;
            }
            for (_, f) in r[y].ifs.iter() {
                if !f.up {
                    continue;
                }
                let mask = if f.is_loop { "255.255.255.255".to_string() } else { f.mask.clone() };
                let netw = if f.is_loop { f.ip.clone() } else { U::network(&f.ip, &f.mask) };
                let k = format!("{}/{}", netw, mask);
                if connected.contains(&k) {
                    continue;
                }
                let metric = dist[y] + f.cost;
                let hop = &first[y];
                let replace = match best.get(&k) {
                    None => true,
                    Some(cur) => metric < cur.metric || (metric == cur.metric && U::ipn(&hop.ip) < U::ipn(&cur.nh)),
                };
                if replace {
                    best.insert(
                        k,
                        DynRoute {
                            kind: "O".into(),
                            net: netw,
                            mask,
                            nh: hop.ip.clone(),
                            iface: hop.iface.clone(),
                            ep_key: format!("E:{}:{}", src, hop.iface),
                            metric,
                            ad: 110,
                            origin: Some(y.clone()),
                            path: vec![],
                            fwd: None,
                        },
                    );
                }
            }
        }
        // Extern defaultrutt (E2, metric 1) från den som har default-information originate
        for y in &ids {
            if y == src || !dist.contains_key(y) {
                continue;
            }
            if !st.dev(y).cfg().ospf.as_ref().map(|o| o.default_originate).unwrap_or(false) || !has_default(y, out) {
                continue;
            }
            let hop = &first[y];
            let k = "0.0.0.0/0.0.0.0".to_string();
            let ok = match best.get(&k) {
                None => true,
                Some(cur) => cur.fwd.map(|f| dist[y] < f).unwrap_or(false),
            };
            if ok {
                best.insert(
                    k,
                    DynRoute {
                        kind: "O*E2".into(),
                        net: "0.0.0.0".into(),
                        mask: "0.0.0.0".into(),
                        nh: hop.ip.clone(),
                        iface: hop.iface.clone(),
                        ep_key: format!("E:{}:{}", src, hop.iface),
                        metric: 1,
                        ad: 110,
                        origin: Some(y.clone()),
                        path: vec![],
                        fwd: Some(dist[y]),
                    },
                );
            }
        }
        for (_, rt) in best {
            out.routes.entry(src.clone()).or_default().push(rt);
        }
        r.get_mut(src).unwrap().dist = dist;
    }
    out.ospf = r;
}

// ------------------------------------------------------------------ BGP (bara eBGP mellan direktanslutna grannar)

struct Speaker {
    as_: i64,
    neighbors: IndexMap<String, BgpNeighbor>,
    ios: bool,
    /// Det en ISP annonserar (path, net, mask)
    advertise: Vec<(String, String, Vec<i64>)>,
    networks: Vec<(String, String)>,
}

fn bgp_speaker(st: &State, id: &str) -> Option<Speaker> {
    let d = st.devices.get(id)?;
    if d.kind == Kind::Router && d.os == Os::Ios {
        if let Some(b) = d.cfg().bgp.as_ref() {
            return Some(Speaker {
                as_: b.as_,
                neighbors: b.neighbors.clone(),
                ios: true,
                advertise: vec![],
                networks: b.networks.iter().map(|n| (n.net.clone(), n.mask.clone())).collect(),
            });
        }
    }
    if d.kind == Kind::Isp {
        if let Some(b) = d.bgp.as_ref() {
            return Some(Speaker {
                as_: b.as_,
                neighbors: b.neighbors.clone(),
                ios: false,
                advertise: b.advertise.iter().map(|a| (a.net.clone(), a.mask.clone(), a.path.clone())).collect(),
                networks: vec![],
            });
        }
    }
    None
}

fn bgp(st: &State, d: &Derived, out: &mut DynOut) {
    let mut res: IndexMap<String, BgpInfo> = IndexMap::new();
    for (id, dev) in st.devices.iter() {
        if !is_router(st, id) {
            continue;
        }
        let cb = match dev.cfg().bgp.as_ref() {
            Some(b) if b.as_ != 0 => b,
            _ => continue,
        };
        let me = bgp_speaker(st, id).unwrap();
        let mut peers: Vec<BgpPeer> = vec![];
        let mut table: Vec<BgpEntry> = vec![];
        for (ip, nb) in me.neighbors.iter() {
            let mut p = BgpPeer { ip: ip.clone(), remote_as: nb.remote_as, state: "Active".into(), ..Default::default() };
            let pi = peers.len();
            peers.push(BgpPeer::default());
            let fin = |p: BgpPeer, peers: &mut Vec<BgpPeer>| peers[pi] = p;
            if nb.shutdown {
                p.state = "Idle (Admin)".into();
                p.reason = Some("admin".into());
                fin(p, &mut peers);
                continue;
            }
            if nb.remote_as.unwrap_or(0) == 0 {
                p.state = "Idle".into();
                p.reason = Some("noas".into());
                fin(p, &mut peers);
                continue;
            }
            let local = eps_of(d, id)
                .into_iter()
                .find(|e| e.up && e.ip.is_some() && !e.is_loop && U::same_subnet(ip, e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("0.0.0.0")));
            let local = match local {
                Some(l) => l,
                None => {
                    p.reason = Some("noroute".into());
                    fin(p, &mut peers);
                    continue;
                }
            };
            p.local_ip = local.ip.clone();
            p.local_iface = Some(local.iface.clone());
            let seg = d.comp.get(&local.key).copied();
            let owner = seg.and_then(|s| d.seg_eps(s).into_iter().find(|e| e.up && e.ip.as_deref() == Some(ip.as_str())));
            let owner = match owner {
                Some(o) => o,
                None => {
                    p.reason = Some("noreply".into());
                    fin(p, &mut peers);
                    continue;
                }
            };
            let peer = match bgp_speaker(st, &owner.dev) {
                Some(x) => x,
                None => {
                    p.reason = Some("refused".into());
                    fin(p, &mut peers);
                    continue;
                }
            };
            let back = peer.neighbors.get(local.ip.as_deref().unwrap_or(""));
            let back = match back {
                Some(b) if !b.shutdown => b,
                _ => {
                    p.reason = Some("refused".into());
                    fin(p, &mut peers);
                    continue;
                }
            };
            if nb.remote_as != Some(peer.as_) {
                p.reason = Some("bad-as".into());
                p.peer_as = Some(peer.as_);
                fin(p, &mut peers);
                continue;
            }
            if back.remote_as != Some(me.as_) {
                p.reason = Some("peer-bad-as".into());
                fin(p, &mut peers);
                continue;
            }
            p.state = "Established".into();
            p.up = true;
            p.peer_dev = Some(owner.dev.clone());
            // Det grannen annonserar
            let mut adv: Vec<(String, String, Vec<i64>)> = vec![];
            if !peer.ios {
                for (n, m, path) in &peer.advertise {
                    let mut pa = vec![peer.as_];
                    pa.extend(path.iter().copied());
                    adv.push((n.clone(), m.clone(), pa));
                }
            } else {
                for (n, m) in &peer.networks {
                    adv.push((n.clone(), m.clone(), vec![peer.as_]));
                }
            }
            for (n, m, path) in adv {
                if path.contains(&me.as_) {
                    continue; // AS-vägen visar att vägen redan gått genom oss
                }
                table.push(BgpEntry {
                    net: n,
                    mask: m,
                    nh: ip.clone(),
                    path,
                    from: Some(ip.clone()),
                    iface: Some(local.iface.clone()),
                    ep_key: Some(local.key.clone()),
                    local: false,
                    best: false,
                });
                p.pfx += 1;
            }
            fin(p, &mut peers);
        }
        // Egna nät (network ... mask ...) finns i tabellen med vägen tom
        for n in cb.networks.iter() {
            table.push(BgpEntry { net: n.net.clone(), mask: n.mask.clone(), nh: "0.0.0.0".into(), path: vec![], from: None, iface: None, ep_key: None, local: true, best: false });
        }
        // Bästa väg per prefix: kortast AS-väg, sedan lägst grannadress
        let mut by_pfx: IndexMap<String, Vec<usize>> = IndexMap::new();
        for (i, r) in table.iter().enumerate() {
            by_pfx.entry(format!("{}/{}", r.net, r.mask)).or_default().push(i);
        }
        for (_, mut list) in by_pfx {
            list.sort_by(|&a, &b| {
                let (ta, tb) = (&table[a], &table[b]);
                let la = if ta.local { -1 } else { 0 };
                let lb = if tb.local { -1 } else { 0 };
                la.cmp(&lb).then(ta.path.len().cmp(&tb.path.len())).then(U::ipn(&ta.nh).cmp(&U::ipn(&tb.nh)))
            });
            let bi = list[0];
            table[bi].best = true;
            let b = table[bi].clone();
            if b.local {
                continue;
            }
            // Anslutna nät vinner ändå på avståndet, men bara de andra installeras
            let conn = eps_of(d, id).iter().any(|e| {
                e.up && e.ip.is_some() && U::network(e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("0.0.0.0")) == b.net && e.mask.as_deref() == Some(b.mask.as_str())
            });
            if !conn {
                out.routes.entry(id.clone()).or_default().push(DynRoute {
                    kind: "B".into(),
                    net: b.net.clone(),
                    mask: b.mask.clone(),
                    nh: b.nh.clone(),
                    iface: b.iface.clone().unwrap_or_default(),
                    ep_key: b.ep_key.clone().unwrap_or_default(),
                    metric: 0,
                    ad: 20,
                    origin: None,
                    path: b.path.clone(),
                    fwd: None,
                });
            }
        }
        let rid = cb.router_id.clone().filter(|r| !r.is_empty()).or_else(|| router_id(st, d, id));
        res.insert(id.clone(), BgpInfo { as_: me.as_, rid, peers, table });
    }
    out.bgp = res;
}

// ------------------------------------------------------------------ Loggrader

/// Loggrader när grannskap och sessioner ändras (minnet ligger i state så att det följer med sparade spel).
/// Raderna köas i out.logs och skrivs först när beräkningen är klar.
fn log_changes(st: &mut State, out: &mut DynOut) {
    let time = st.time;
    let old_ep_up = |st: &State, id: &str, ifn: &str| -> bool {
        st.cache.as_ref().and_then(|c| c.eps.get(&format!("E:{}:{}", id, ifn))).map(|e| e.up).unwrap_or(false)
    };
    let ready = st.dyn_mem.ready;
    let mut q: Vec<(String, String)> = vec![];
    let ids: Vec<String> = out.ospf.keys().cloned().collect();
    for id in &ids {
        let r = &out.ospf[id];
        let pid = st.dev(id).cfg().ospf.as_ref().map(|o| o.pid).unwrap_or(1);
        let mut now: Vec<String> = vec![];
        for nb in r.nbrs.iter() {
            let k = format!("{}@{}", nb.rid, nb.iface);
            now.push(k.clone());
            let mk = format!("{}|{}", id, k);
            if !st.dyn_mem.ospf.get(&mk).copied().unwrap_or(false) {
                if ready {
                    q.push((id.clone(), format!("%OSPF-5-ADJCHG: Process {}, Nbr {} on {} from LOADING to FULL, Loading Done", pid, nb.rid, nb.iface)));
                }
                st.dyn_mem.since.insert(mk, if ready { time } else { time - 2317.0 });
            }
        }
        let prefix = format!("{}|", id);
        let mut keys: Vec<String> = st.dyn_mem.ospf.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        keys.sort();
        for mk in keys {
            let k = mk[id.len() + 1..].to_string();
            let present = now.contains(&k);
            if !present && ready {
                let mut parts = k.splitn(2, '@');
                let nrid = parts.next().unwrap_or("");
                let ifn = parts.next().unwrap_or("");
                let why = if old_ep_up(st, id, ifn) { "Dead timer expired" } else { "Interface down or detached" };
                q.push((id.clone(), format!("%OSPF-5-ADJCHG: Process {}, Nbr {} on {} from FULL to DOWN, Neighbor Down: {}", pid, nrid, ifn, why)));
            }
            if !present {
                st.dyn_mem.ospf.remove(&mk);
            }
        }
        for k in now {
            st.dyn_mem.ospf.insert(format!("{}|{}", id, k), true);
        }
        // Fel area loggas av IOS; olika timers syns bara med debug
        for p in r.problems.iter() {
            let pk = format!("p|{}|{}|{}", id, p.iface, p.why);
            if st.dyn_mem.ospf.get(&pk).copied().unwrap_or(false) {
                continue;
            }
            st.dyn_mem.ospf.insert(pk, true);
            if !ready {
                continue;
            }
            if p.why == "area" {
                q.push((id.clone(), format!("%OSPF-4-ERRRCV: Received invalid packet: mismatched area ID from backbone area must be virtual-link but not found from {}, {}", p.peer_ip, p.iface)));
            }
            if p.why == "rid" {
                q.push((id.clone(), format!("%OSPF-4-DUP_RTRID_NBR: OSPF detected duplicate router-id {} from {} on interface {}", r.rid.clone().unwrap_or_default(), p.peer_ip, p.iface)));
            }
        }
    }
    for (id, info) in out.bgp.iter() {
        for p in info.peers.iter() {
            let k = format!("{}|{}", id, p.ip);
            let was = st.dyn_mem.bgp.get(&k).cloned();
            if p.up && was.as_deref() != Some("up") {
                if ready {
                    q.push((id.clone(), format!("%BGP-5-ADJCHANGE: neighbor {} Up", p.ip)));
                }
                st.dyn_mem.since.insert(format!("b|{}", k), if ready { time } else { time - 1201.0 });
            }
            if !p.up && was.as_deref() == Some("up") && ready {
                q.push((id.clone(), format!("%BGP-5-ADJCHANGE: neighbor {} Down Peer closed the session", p.ip)));
            }
            if p.reason.as_deref() == Some("bad-as") && was.as_deref() != Some("bad-as") && ready {
                let hex = format!("{:X}", p.peer_as.unwrap_or(0));
                let hex = format!("{:0>4}", hex);
                let hex = &hex[hex.len() - 4..];
                q.push((id.clone(), format!("%BGP-3-NOTIFICATION: sent to neighbor {} passive 2/2 (peer in wrong AS) 2 bytes {}", p.ip, hex)));
            }
            st.dyn_mem.bgp.insert(k, if p.up { "up".into() } else { p.reason.clone().unwrap_or_else(|| "down".into()) });
        }
    }
    st.dyn_mem.ready = true;
    out.logs.extend(q);
}

/// Räknar BGP och sedan OSPF på D (som redan har eps, comp och by_seg). Sätter d.dynr.
/// quiet = inga loggrader för grannskap.
pub fn compute(st: &mut State, d: &mut Derived, quiet: bool) {
    let mut out = DynOut::default();
    // BGP först: en BGP-default kan spridas vidare med OSPF
    bgp(st, d, &mut out);
    ospf(st, d, &mut out);
    // Tyst när en kontroll bara provar något tillfälligt
    if !quiet && !st.quiet {
        log_changes(st, &mut out);
    }
    d.dynr = out;
}

/// Skriver köade loggrader (d.dynr.logs) med sim::push_log
pub fn flush(st: &mut State, d: &mut Derived) {
    let q = std::mem::take(&mut d.dynr.logs);
    for (dev, msg) in q {
        sim::push_log(st, &dev, &msg);
    }
}

pub fn hello(i: Option<&crate::net::model::Iface>) -> i64 {
    i.and_then(|i| i.ospf_hello).unwrap_or(10)
}
pub fn dead(i: Option<&crate::net::model::Iface>) -> i64 {
    i.and_then(|i| i.ospf_dead).unwrap_or_else(|| hello(i) * 4)
}
/// Sedan när grannskapet/sessionen funnits (dynMem.since)
pub fn since(st: &State, key: &str) -> f64 {
    st.dyn_mem.since.get(key).copied().unwrap_or(0.0)
}
