//! Kurs 2 (vecka 11–14): Mölndal, OSPF, BGP och VPN. Port av js/kurs2.js (felen och kontrollerna).
#![allow(unused_variables, dead_code)]
use super::*;

fn fwd(st: &mut State, from: &str, to: &str) -> bool {
    sim::traceroute(st, from, to, None).0.ok
}
fn via(st: &mut State, from: &str, to: &str, ip: &str) -> bool {
    let f = sim::traceroute(st, from, to, None).0;
    f.ok && f.hops.iter().any(|h| h.ip == ip)
}
fn look(st: &mut State, dev: &str, ip: &str) -> Option<RouteHit> {
    let d = sim::get(st);
    sim::route_lookup(st, &d, dev, ip)
}
use crate::net::sim::RouteHit;
/// Prova något med ett interface nere, och ställ tillbaka det efteråt
fn with_down(st: &mut State, dev: &str, ifn: &str, f: impl FnOnce(&mut State) -> bool) -> bool {
    let was = st.dev(dev).cfg().ifaces[ifn].shutdown;
    st.quiet = true;
    st.dev_mut(dev).cfg_mut().ifaces.get_mut(ifn).unwrap().shutdown = true;
    sim::touch(st);
    let r = f(st);
    st.dev_mut(dev).cfg_mut().ifaces.get_mut(ifn).unwrap().shutdown = was;
    sim::touch(st);
    sim::get(st);
    st.quiet = false;
    r
}
fn ospf_nbrs(st: &mut State, dev: &str) -> Vec<String> {
    let d = sim::get(st);
    d.dynr.ospf.get(dev).map(|r| r.nbrs.iter().map(|n| n.rid.clone()).collect()).unwrap_or_default()
}
fn bgp_peer(st: &mut State, dev: &str, ip: &str) -> Option<crate::net::dynroute::BgpPeer> {
    let d = sim::get(st);
    d.dynr.bgp.get(dev).and_then(|b| b.peers.iter().find(|p| p.ip == ip).cloned())
}
fn static_cover(c: &IosConfig, dst: &str, nh: &str, min_ad: i64) -> bool {
    c.routes.iter().any(|r| r.nh.as_deref() == Some(nh) && r.ad.unwrap_or(1) >= min_ad && U::in_net(dst, &r.net, &r.mask) && r.net != "0.0.0.0")
}

// ------------------------------------------------------------------ Vecka 11
pub(crate) fn su_w11(st: &mut State) {
    // Fel 1: Mölndals defaultrutt pekar på en adress som inte ligger i något anslutet nät
    both(st.dev_mut("RM"), |c| c.routes = vec![Route { net: "0.0.0.0".into(), mask: "0.0.0.0".into(), nh: Some("10.0.0.1".into()), iface: None, ad: None }]);
    // Fel 2: navets rutt till Mölndal pekar mot Borås, och Borås defaultrutt skickar tillbaka: en loop
    both(st.dev_mut("R1"), |c| {
        for r in c.routes.iter_mut() {
            if r.net == "192.168.3.0" {
                r.nh = Some("10.0.0.2".into());
            }
        }
    });
}
pub(crate) fn ck_v11k1(st: &mut State) -> bool { fwd(st, "PC-Molndal", "192.168.1.10") }
pub(crate) fn ck_v11k2(st: &mut State) -> bool { fwd(st, "PC-Lager", "192.168.3.10") && fwd(st, "PC-Anna", "192.168.3.10") }
pub(crate) fn ck_v11n1(st: &mut State) -> bool {
    if !static_cover(st.dev("RB").cfg(), "192.168.3.10", "10.0.0.10", 2) || !static_cover(st.dev("RM").cfg(), "192.168.2.1", "10.0.0.9", 2) {
        return false;
    }
    if !via(st, "PC-Lager", "192.168.3.10", "10.0.0.1") || !via(st, "PC-Molndal", "192.168.2.1", "10.0.0.5") {
        return false;
    }
    // Länken Borås–Göteborg nere: Borås ska ta direktvägen
    with_down(st, "RB", "GigabitEthernet0/2", |st| {
        look(st, "RB", "192.168.3.10").map(|r| r.nh.as_deref() == Some("10.0.0.10")).unwrap_or(false)
    })
}

// ------------------------------------------------------------------ Vecka 12
pub(crate) fn su_w12(st: &mut State) {
    // Fel 1: Borås kör DH-grupp 2, Göteborg grupp 5
    both(st.dev_mut("RB"), |c| c.crypto.as_mut().unwrap().policies.get_mut(&10).unwrap().group = 2);
    // Fel 2: NAT-undantaget i Göteborg pekar på Mölndals nät i stället för lagrets
    both(st.dev_mut("R1"), |c| c.acls.get_mut("NAT-UT").unwrap().rules[0].dst = Some(AddrSpec::net("192.168.3.0", "0.0.0.63")));
}
pub(crate) fn af_w12(st: &mut State) {
    sim::ping(st, "PC-Lager", "192.168.1.10", PingOpts::default());
    sim::ping(st, "Tekniker", "192.168.2.193", PingOpts::default());
    sim::push_log(st, "R1", "%PARSER-5-CFGLOG_LOGGEDCMD: User:konsult  logged command:ip access-list extended NAT-UT");
    sim::push_log(st, "R1", "%PARSER-5-CFGLOG_LOGGEDCMD: User:konsult  logged command:10 deny ip 192.168.1.0 0.0.0.63 192.168.3.0 0.0.0.63");
}
pub(crate) fn ck_v12k1(st: &mut State) -> bool {
    sim::ping(st, "Tekniker", "192.168.2.193", PingOpts::default());
    let c = sim::crypto_status(st, "R1");
    c.isakmp.iter().any(|x| x.dst == "203.0.113.20" && x.state == "QM_IDLE")
}
pub(crate) fn ck_v12k2(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Anna", "192.168.2.1") }
pub(crate) fn ck_v12n1(st: &mut State) -> bool {
    !ok_with(st, "PC-Molndal", "192.168.1.70", PingOpts::tcp(443))
        && ok_with(st, "PC-Bo", "192.168.1.70", PingOpts::tcp(443))
        && ok_with(st, "PC-Molndal", "192.168.1.10", PingOpts::tcp(445))
        && ok(st, "PC-Molndal", "9.9.9.9")
}

// ------------------------------------------------------------------ Vecka 13
pub(crate) fn su_w13(st: &mut State) {
    // Fel 1 a: Borås länkar ligger i area 1, grannarna i area 0
    both(st.dev_mut("RB"), |c| {
        if let Some(o) = c.ospf.as_mut() {
            for n in o.networks.iter_mut() {
                if n.net != "192.168.2.0" {
                    n.area = 1;
                }
            }
        }
    });
    // Fel 1 b: Mölndals sida av länken mot Borås har andra timers
    both(st.dev_mut("RM"), |c| {
        let i = iface(c, "GigabitEthernet0/1");
        i.ospf_hello = Some(30);
        i.ospf_dead = Some(120);
    });
    // Fel 2: bandwidth 100000 på Göteborgs serielänk
    both(st.dev_mut("R1"), |c| iface(c, "Serial0/0/0").bandwidth = Some(100000));
}
pub(crate) fn ck_v13k1(st: &mut State) -> bool {
    let n = ospf_nbrs(st, "RB");
    n.iter().any(|r| r == "1.1.1.1") && n.iter().any(|r| r == "3.3.3.3")
}
pub(crate) fn ck_v13k2(st: &mut State) -> bool {
    look(st, "R1", "192.168.3.10").map(|r| r.kind == "O" && r.nh.as_deref() == Some("10.0.0.2") && r.metric == 3).unwrap_or(false)
}
pub(crate) fn ck_v13n1(st: &mut State) -> bool {
    let d = sim::get(st);
    let dynr = d.dynr.routes.get("R1").map(|v| v.iter().any(|x| x.kind == "O" && x.mask == "255.255.255.255" && x.origin.as_deref() == Some("RM"))).unwrap_or(false);
    let inst = sim::installed_routes(st, &d, "R1").iter().any(|x| x.kind == "O" && x.mask == "255.255.255.255");
    let any_static = ["R1", "RB"].iter().any(|id| st.dev(id).cfg().routes.iter().any(|q| q.mask == "255.255.255.255"));
    dynr && inst && !any_static
}

// ------------------------------------------------------------------ Vecka 14
pub(crate) fn su_w14(st: &mut State) {
    // Fel 1: fel AS-nummer på grannen ISP-A (64050 i stället för 64500)
    both(st.dev_mut("R1"), |c| c.bgp.as_mut().unwrap().neighbors.get_mut("203.0.113.1").unwrap().remote_as = Some(64050));
    // Fel 2: Gi0/2 mot Borås saknar ip nat inside
    both(st.dev_mut("R1"), |c| iface(c, "GigabitEthernet0/2").nat_dir = None);
}
pub(crate) fn ck_v14k1(st: &mut State) -> bool { bgp_peer(st, "R1", "203.0.113.1").map(|p| p.up).unwrap_or(false) }
pub(crate) fn ck_v14k2(st: &mut State) -> bool {
    ok(st, "PC-Lager", "9.9.9.9") && ok_with(st, "PC-Molndal", "198.51.100.80", PingOpts::tcp(80))
}
pub(crate) fn ck_v14n1(st: &mut State) -> bool {
    if !bgp_peer(st, "R1", "198.51.100.1").map(|p| p.up).unwrap_or(false) {
        return false;
    }
    if !ok_with(st, "PC-Molndal", "198.51.100.80", PingOpts::tcp(80)) || !ok(st, "PC-Anna", "9.9.9.9") {
        return false;
    }
    with_down(st, "R1", "GigabitEthernet0/1", |st| {
        ok(st, "PC-Anna", "9.9.9.9") && ok_with(st, "PC-Molndal", "198.51.100.80", PingOpts::tcp(80))
    })
}
