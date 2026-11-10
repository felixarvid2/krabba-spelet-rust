//! Byggena 1–5 (vecka 101–105): enheterna kommer fabriksinställda. Port av js/build.js (kontrollerna och felen).
#![allow(unused_variables, dead_code)]
use super::*;

/// Nätet byggena utgår från (vecka 9 eller 10)
pub(crate) fn build_base(n: u32) -> u32 {
    if n == 105 { 10 } else { 9 }
}

/// Som en riktig dator försöker DHCP-klienten igen om den saknar en giltig adress
fn lease(st: &mut State, host: &str, net: &str, mask: &str) -> bool {
    if !st.devices.contains_key(host) {
        return false;
    }
    let good = |st: &State| {
        sim::host_ip_conf(st.dev(host)).map(|c| !c.apipa && c.ip.as_deref().map(|ip| U::in_net(ip, net, mask)).unwrap_or(false)).unwrap_or(false)
    };
    if !good(st) {
        sim::dhcp(st, host);
    }
    good(st)
}
fn ifc<'a>(st: &'a State, dev: &str, n: &str) -> Option<&'a Iface> {
    st.dev(dev).cfg().ifaces.get(n)
}
fn cfg<'a>(st: &'a State, dev: &str) -> &'a IosConfig {
    st.dev(dev).cfg()
}
/// Nollställ en IOS-enhet till fabriksinställningar: ingen konfiguration och inget sparat
pub(crate) fn wipe(st: &mut State, id: &str) {
    let fresh = factory_config(st.dev(id));
    let d = st.dev_mut(id);
    d.config = Some(fresh);
    d.startup = None;
    d.rt.dhcp_bindings.clear();
    d.rt.nat_trans.clear();
    d.rt.errdisabled.clear();
    d.rt.acl_counters.clear();
    d.rt.logs.clear();
    d.rt.ike.clear();
    d.rt.ipsec.clear();
}
pub(crate) fn renew_all(st: &mut State) {
    for h in DHCP_HOSTS {
        if st.devices.contains_key(*h) {
            sim::dhcp(st, h);
        }
    }
}
fn vlan_named(c: &IosConfig, id: u16, name: &str) -> bool {
    c.vlans.get(&id).map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false)
}
fn access(c: &IosConfig, port: u32, vlan: u16) -> bool {
    c.ifaces.get(&format!("GigabitEthernet0/{}", port)).map(|i| i.mode == "access" && i.access_vlan == vlan).unwrap_or(false)
}
fn ssh_ok(st: &State, dev: &str) -> bool {
    let d = st.dev(dev);
    sim::ssh_enabled(d).is_some() && sim::vty_allows(d, "ssh") && !sim::vty_allows(d, "telnet") && d.cfg().users.contains_key("drift")
}
fn same_pol(a: &IsakmpPolicy, b: &IsakmpPolicy) -> bool {
    a.enc == b.enc && a.hash == b.hash && a.auth == b.auth && a.group == b.group
}

// ------------------------------------------------------------------ Bygge 1
pub(crate) fn ck_b1a(st: &mut State) -> bool {
    let d = st.dev("SW1");
    d.cfg().hostname == "SW-Nordvik-1" && d.startup.as_ref().map(|s| s.hostname == "SW-Nordvik-1").unwrap_or(false)
}
pub(crate) fn ck_b1b(st: &mut State) -> bool {
    let c = cfg(st, "SW1");
    vlan_named(c, 10, "kontor") && vlan_named(c, 20, "ekonomi") && vlan_named(c, 30, "gast") && vlan_named(c, 99, "drift")
}
pub(crate) fn ck_b1c(st: &mut State) -> bool {
    let c = cfg(st, "SW1");
    access(c, 5, 10) && access(c, 6, 10) && access(c, 11, 10) && access(c, 12, 10) && access(c, 7, 20) && access(c, 8, 20) && access(c, 9, 30) && access(c, 10, 99) && access(c, 20, 99)
}
pub(crate) fn ck_b1d(st: &mut State) -> bool {
    lease(st, "PC-Anna", "192.168.1.0", "255.255.255.192")
        && lease(st, "PC-Bo", "192.168.1.64", "255.255.255.192")
        && ok(st, "PC-Anna", "192.168.1.10")
        && ok(st, "Tekniker", "192.168.1.193")
}
pub(crate) fn ck_b1e(st: &mut State) -> bool {
    ssh_ok(st, "SW1") && ok(st, "Tekniker", "192.168.1.194") && ok(st, "SW1", "192.168.1.10")
}

// ------------------------------------------------------------------ Bygge 2
pub(crate) fn ck_b2a(st: &mut State) -> bool {
    ok(st, "Tekniker", "192.168.1.193") && ok(st, "Filserver", "192.168.1.1") && ok(st, "Ekonomisystem", "192.168.1.65") && ok(st, "Filserver", "192.168.1.70")
}
pub(crate) fn ck_b2b(st: &mut State) -> bool {
    if !lease(st, "PC-Anna", "192.168.1.0", "255.255.255.192")
        || !lease(st, "PC-Bo", "192.168.1.64", "255.255.255.192")
        || !lease(st, "Gast-laptop", "192.168.1.128", "255.255.255.192")
    {
        return false;
    }
    let a = sim::host_ip_conf(st.dev("PC-Anna")).unwrap();
    a.gw.as_deref() == Some("192.168.1.1") && !a.dns.is_empty() && ok(st, "PC-Anna", "192.168.1.1")
}
pub(crate) fn ck_b2c(st: &mut State) -> bool {
    let r = sim::resolve(st, "PC-Anna", "filserver");
    r.error.is_none() && r.ip.as_deref() == Some("192.168.1.10")
}
pub(crate) fn ck_b2d(st: &mut State) -> bool {
    let stat = cfg(st, "R1").nat.statics.iter().any(|s| s.local == "192.168.1.10" && s.global == "203.0.113.11");
    ok(st, "PC-Anna", "198.51.100.80") && ok(st, "Gast-laptop", "198.51.100.80") && stat
}
pub(crate) fn ck_b2e(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Lager", "198.51.100.80") }
pub(crate) fn ck_b2f(st: &mut State) -> bool {
    cfg(st, "R1").hostname == "R-Nordvik-1" && ssh_ok(st, "R1") && ok(st, "Tekniker", "192.168.1.193")
}

// ------------------------------------------------------------------ Bygge 3
pub(crate) fn ck_b3a(st: &mut State) -> bool {
    let c = cfg(st, "SWB");
    c.hostname == "SW-Boras-1" && vlan_named(c, 40, "lager") && c.vlans.contains_key(&50) && vlan_named(c, 99, "drift") && access(c, 5, 40)
}
pub(crate) fn ck_b3b(st: &mut State) -> bool {
    if sim::op_mode(st, "SWB", "GigabitEthernet0/24") != "trunk" {
        return false;
    }
    (1..=3).all(|n| {
        let p = format!("GigabitEthernet0/{}", n);
        sim::op_mode(st, "SWB", &p) == "trunk" && cfg(st, "SWB").ifaces[&p].native == 99
    })
}
pub(crate) fn ck_b3c(st: &mut State) -> bool {
    ok(st, "AP-Lager-1", "192.168.2.193") && ok(st, "AP-Lager-1", "192.168.1.196") && ok(st, "RB", "192.168.1.10")
}
pub(crate) fn ck_b3d(st: &mut State) -> bool {
    lease(st, "PC-Lager", "192.168.2.0", "255.255.255.192") && ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Lager", "198.51.100.80")
}
pub(crate) fn ck_b3e(st: &mut State) -> bool {
    let d = sim::get(st);
    let j = |n: &str| d.ap_joined.get(n).copied().unwrap_or(false);
    j("AP-Lager-1") && j("AP-Lager-2") && j("AP-Lager-3") && lease(st, "Lager-skanner", "192.168.2.0", "255.255.255.192") && lease(st, "Gast-mobil", "192.168.2.64", "255.255.255.192")
}
pub(crate) fn ck_b3f(st: &mut State) -> bool {
    let has = ifc(st, "RB", "GigabitEthernet0/0.50").map(|i| i.acl_in.is_some()).unwrap_or(false);
    has && lease(st, "Gast-mobil", "192.168.2.64", "255.255.255.192") && !ok(st, "Gast-mobil", "192.168.1.10") && ok(st, "Gast-mobil", "198.51.100.80")
}

// ------------------------------------------------------------------ Bygge 4
pub(crate) fn su_b4(st: &mut State) {
    both(st.dev_mut("R1"), |c| {
        c.acls.shift_remove("GAST");
        c.acls.shift_remove("KONTOR-UT");
        c.ifaces.get_mut("GigabitEthernet0/0.30").unwrap().acl_in = None;
        c.ifaces.get_mut("GigabitEthernet0/0.10").unwrap().acl_in = None;
    });
    both(st.dev_mut("SW1"), |c| c.ifaces.get_mut("GigabitEthernet0/9").unwrap().port_sec = None);
    for id in ["R1", "SW1", "SW2"] {
        both(st.dev_mut(id), |c| {
            c.ntp_servers = vec![];
            c.logging.hosts = vec![];
            c.timestamps_msec = false;
        });
    }
}
pub(crate) fn ck_b4a(st: &mut State) -> bool {
    let has = ifc(st, "R1", "GigabitEthernet0/0.30").map(|i| i.acl_in.is_some()).unwrap_or(false);
    has && !ok(st, "Gast-laptop", "192.168.1.10") && ok(st, "Gast-laptop", "198.51.100.80")
}
pub(crate) fn ck_b4b(st: &mut State) -> bool {
    !ok(st, "PC-Anna", "192.168.1.70") && ok(st, "PC-Anna", "198.51.100.80") && ok(st, "PC-Anna", "192.168.1.10") && ok(st, "PC-Bo", "192.168.1.70")
}
pub(crate) fn ck_b4c(st: &mut State) -> bool {
    let good = match ifc(st, "SW1", "GigabitEthernet0/9").and_then(|i| i.port_sec.as_ref()) {
        Some(p) => !p.disabled && p.max == 1 && p.violation == "shutdown",
        None => false,
    };
    good && !st.dev("SW1").rt.errdisabled.contains_key("GigabitEthernet0/9") && ok(st, "Gast-laptop", "198.51.100.80")
}
pub(crate) fn ck_b4d(st: &mut State) -> bool {
    ["R1", "SW1", "SW2"].iter().all(|id| sim::ntp_synced(st, id))
}
pub(crate) fn ck_b4e(st: &mut State) -> bool {
    ["R1", "SW1", "SW2"].iter().all(|id| {
        let c = cfg(st, id);
        c.logging.hosts.iter().any(|h| h == "192.168.1.17") && c.timestamps_msec
    })
}

// ------------------------------------------------------------------ Bygge 5
pub(crate) fn su_b5(st: &mut State) {
    for id in ["R1", "RB"] {
        both(st.dev_mut(id), |c| {
            c.crypto = None;
            c.acls.shift_remove("VPN-TRAFIK");
            c.acls.get_mut("NAT-UT").unwrap().rules.retain(|r| r.action == "permit");
            for i in c.ifaces.values_mut() {
                i.crypto_map = None;
                i.adjust_mss = None;
            }
        });
    }
}
pub(crate) fn ck_b5a(st: &mut State) -> bool {
    let (Some(a), Some(b)) = (cfg(st, "R1").crypto.as_ref(), cfg(st, "RB").crypto.as_ref()) else { return false };
    let pol = a.policies.values().any(|x| x.auth == "pre-share" && b.policies.values().any(|y| same_pol(x, y)));
    let ka = a.keys.get("203.0.113.20").map(|s| s.as_str()).unwrap_or("");
    let kb = b.keys.get("203.0.113.10").map(|s| s.as_str()).unwrap_or("");
    pol && !ka.is_empty() && ka == kb
}
pub(crate) fn ck_b5b(st: &mut State) -> bool {
    let (a, b) = (cfg(st, "R1"), cfg(st, "RB"));
    let (Some(ca), Some(cb)) = (a.crypto.as_ref(), b.crypto.as_ref()) else { return false };
    let ts = ca.transform_sets.values().any(|x| cb.transform_sets.values().any(|y| y.transforms.join(" ") == x.transforms.join(" ")));
    let (Some(ma), Some(mb)) = (a.acls.get("VPN-TRAFIK"), b.acls.get("VPN-TRAFIK")) else { return false };
    if !ts {
        return false;
    }
    let need = Ace::ext(0, "permit", AddrSpec::net("192.168.1.0", "0.0.0.63"), AddrSpec::net("192.168.2.0", "0.0.0.63"));
    ma.rules.iter().any(|r| sim::mirrors(&need, &Ace { action: r.action.clone(), proto: r.proto.clone(), src: r.dst.clone(), dst: r.src.clone(), ..Default::default() }))
        && mb.rules.iter().any(|r| sim::mirrors(&need, r))
}
pub(crate) fn ck_b5c(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Anna", "192.168.2.1") }
pub(crate) fn ck_b5d(st: &mut State) -> bool {
    let d = sim::get(st);
    d.ap_joined.get("AP-Lager-1").copied().unwrap_or(false) && ok(st, "Tekniker", "192.168.2.193")
}
pub(crate) fn ck_b5e(st: &mut State) -> bool { sim::big_transfer(st, "PC-Lager", "192.168.1.10", 445).ok }
