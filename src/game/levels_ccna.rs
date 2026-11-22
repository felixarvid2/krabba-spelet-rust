//! CCNA-läget (modul 1–8, vecka 201–208): övningar och frågesporter som täcker
//! examensområdena i Ciscos CCNA 200-301 på Nordviks nät.
//! Modulerna 1–4 och 6–8 bygger på kurs 1-nätet (vecka 9), modul 5 (OSPF) på kurs 2-nätet (vecka 14).
use super::*;
use crate::net::sim::push_log;

/// Vilket felfritt nät (och vilken värld) en CCNA-modul bygger på
pub(super) fn base(n: u32) -> u32 {
    if n == 205 { 14 } else { 9 }
}

// ------------------------------------------------------------------ Hjälpfunktioner

fn ospf_nbrs(st: &mut State, dev: &str) -> Vec<String> {
    let d = sim::get(st);
    d.dynr.ospf.get(dev).map(|r| r.nbrs.iter().map(|n| n.rid.clone()).collect()).unwrap_or_default()
}
fn port_ok(st: &mut State, key: &str) -> bool {
    let d = sim::get(st);
    d.ports.get(key).map(|p| p.up && !p.mismatch).unwrap_or(false)
}
fn errdis(st: &State, dev: &str, port: &str) -> bool {
    st.dev(dev).rt.errdisabled.get(port).map(|s| !s.is_empty()).unwrap_or(false)
}
fn misc(st: &State, k: &str) -> bool {
    st.flags.misc.get(k).copied().unwrap_or(false)
}
fn gx<'a>(c: &'a IosConfig, k: &str) -> Option<&'a str> {
    c.x.get(k).map(|s| s.as_str())
}
/// VLAN-listan sist i en rad som "ip dhcp snooping vlan 10,20-30"
fn vlans_in(line: &str) -> Vec<u16> {
    let last = line.split_whitespace().last().unwrap_or("");
    let mut v = vec![];
    for part in last.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<u16>(), b.trim().parse::<u16>()) {
                v.extend(a..=b);
            }
        } else if let Ok(x) = part.trim().parse::<u16>() {
            v.push(x);
        }
    }
    v
}
fn syslog_level(s: &str) -> Option<u8> {
    let names = ["emergencies", "alerts", "critical", "errors", "warnings", "notifications", "informational", "debugging"];
    if let Ok(n) = s.parse::<u8>() {
        return (n <= 7).then_some(n);
    }
    names.iter().position(|x| x.starts_with(&s.to_lowercase()) && !s.is_empty()).map(|p| p as u8)
}
fn lacp(mode: &str) -> bool {
    mode == "active" || mode == "passive"
}
fn wlc_mut(st: &mut State) -> &mut Wlc {
    st.dev_mut("WLC").wlc.as_mut().unwrap()
}
fn wlc_save(w: &mut Wlc) {
    w.saved = Some(WlcSaved { wlans: w.wlans.clone(), ap_channels: w.ap_channels.clone() });
}

// ------------------------------------------------------------------ Felen

pub(super) fn setup(n: u32, st: &mut State) {
    match n {
        201 => {
            // Saras port låst till 100/full medan datorn förhandlar (och väljer halv duplex)
            both(st.dev_mut("SW1"), |c| {
                let i = iface(c, "GigabitEthernet0/12");
                i.speed = "100".into();
                i.duplex = "full".into();
            });
            // Lisa har fått en statisk adress med ekonominätets gateway
            {
                let nic = st.dev_mut("PC-Lisa").nic.as_mut().unwrap();
                nic.dhcp = false;
                nic.static_ = Some(StaticIp {
                    ip: "192.168.1.21".into(),
                    mask: "255.255.255.192".into(),
                    gw: Some("192.168.1.65".into()),
                    dns: vec!["192.168.1.1".into()],
                });
            }
            sim::release(st, "PC-Lisa");
        }
        202 => {
            both(st.dev_mut("SW2"), |c| {
                c.vlans.remove(&20);
            });
            for (sw, mode) in [("SW1", "active"), ("SW2", "on")] {
                both(st.dev_mut(sw), |c| {
                    for p in ["GigabitEthernet0/24", "GigabitEthernet0/25"] {
                        iface(c, p).ch_group = Some(ChGroup { n: 1, mode: mode.into() });
                    }
                });
            }
            push_log(st, "SW1", "%EC-5-L3DONTBNDL2: Gi0/24 suspended: LACP currently not enabled on the remote port.");
            push_log(st, "SW1", "%EC-5-L3DONTBNDL2: Gi0/25 suspended: LACP currently not enabled on the remote port.");
        }
        203 => {
            both(st.dev_mut("SW2"), |c| {
                c.stp_priority.insert(10, 4096);
            });
            both(st.dev_mut("SW1"), |c| {
                iface(c, "GigabitEthernet0/6").x.insert("bpduguard".into(), "spanning-tree bpduguard enable".into());
            });
            st.dev_mut("SW1").rt.errdisabled.insert("GigabitEthernet0/6".into(), "bpduguard".into());
            push_log(st, "SW1", "%SPANTREE-2-BLOCK_BPDUGUARD: Received BPDU on port Gi0/6 with BPDU Guard enabled. Disabling port.");
            push_log(st, "SW1", "%PM-4-ERR_DISABLE: bpduguard error detected on Gi0/6, putting Gi0/6 in err-disable state");
        }
        204 => {
            both(st.dev_mut("R1"), |c| {
                c.routes.push(Route { net: "192.168.2.0".into(), mask: "255.255.255.192".into(), nh: Some("203.0.113.1".into()), iface: None, ad: None });
                for r in c.routes.iter_mut() {
                    if r.net == "0.0.0.0" {
                        r.ad = Some(250);
                    }
                }
                c.routes.push(Route { net: "0.0.0.0".into(), mask: "0.0.0.0".into(), nh: Some("10.0.0.2".into()), iface: None, ad: None });
            });
        }
        205 => {
            both(st.dev_mut("R1"), |c| {
                if let Some(o) = c.ospf.as_mut() {
                    o.no_passive.retain(|x| x != "Serial0/0/0");
                }
            });
            both(st.dev_mut("RB"), |c| {
                if let Some(o) = c.ospf.as_mut() {
                    o.router_id = Some("1.1.1.1".into());
                }
            });
        }
        206 => {
            both(st.dev_mut("R1"), |c| {
                for s in c.nat.statics.iter_mut() {
                    if s.global == "203.0.113.11" {
                        s.local = "192.168.1.11".into();
                    }
                }
            });
            both(st.dev_mut("SW1"), |c| {
                c.logging.hosts.clear();
                c.x.insert("logging trap".into(), "logging trap critical".into());
            });
        }
        207 => {
            both(st.dev_mut("SW1"), |c| {
                iface(c, "GigabitEthernet0/6").port_sec =
                    Some(PortSec { max: 1, violation: "shutdown".into(), sticky: true, macs: vec!["001a2b3c4d5e".into()], disabled: false });
            });
            both(st.dev_mut("R1"), |c| {
                c.acls.insert(
                    "VTY-SKYDD".into(),
                    Acl {
                        extended: false,
                        rules: vec![Ace {
                            seq: 10,
                            remark: None,
                            action: "permit".into(),
                            proto: None,
                            src: Some(AddrSpec::net("192.168.1.0", "0.0.0.63")),
                            dst: None,
                            dport: None,
                            dport2: None,
                            dop: None,
                            log: false,
                        }],
                    },
                );
                c.lines.vty0_4.access_class = Some("VTY-SKYDD".into());
                let sn = c.snmp.get_or_insert(Snmp { communities: Default::default(), location: None, contact: None });
                sn.communities.insert("public".into(), "RW".into());
            });
        }
        208 => {
            let w = wlc_mut(st);
            w.wlans.get_mut(&2).unwrap().security = "Open".into();
            w.ap_channels.insert("AP-Lager-2".into(), 3);
            wlc_save(w);
        }
        _ => {}
    }
}

pub(super) fn after(n: u32, st: &mut State) {
    if n == 205 {
        ping_ignore(st, "PC-Lager", "192.168.1.10");
    }
}

// ------------------------------------------------------------------ Kontrollerna

// Modul 1
pub(crate) fn ck_c1k1(st: &mut State) -> bool { port_ok(st, "SW1|GigabitEthernet0/12") && ok(st, "PC-Sara", "192.168.1.10") }
pub(crate) fn ck_c1k2(st: &mut State) -> bool { ok(st, "PC-Lisa", "198.51.100.80") && ok(st, "PC-Lisa", "192.168.1.10") }
pub(crate) fn ck_c1n1(st: &mut State) -> bool {
    let c = st.dev("R1").cfg();
    let routing = c.x.contains_key("ipv6 unicast-routing");
    let addr = c.ifaces.get("GigabitEthernet0/0.10").and_then(|i| i.x.get("ipv6 ")).map(|v| v.to_lowercase());
    routing && addr.map(|a| a.trim_end() == "ipv6 address 2001:db8:acad:10::1/64").unwrap_or(false)
}
// Modul 2
pub(crate) fn ck_c2k1(st: &mut State) -> bool { sim::vlan_exists(st.dev("SW2"), 20) && ok(st, "PC-Bo", "192.168.1.70") }
pub(crate) fn ck_c2k2(st: &mut State) -> bool {
    let mut modes = vec![];
    for sw in ["SW1", "SW2"] {
        let c = st.dev(sw).cfg();
        let g: Vec<_> = ["GigabitEthernet0/24", "GigabitEthernet0/25"].iter().map(|p| c.ifaces.get(*p).and_then(|i| i.ch_group.clone())).collect();
        match (&g[0], &g[1]) {
            (Some(a), Some(b)) if a.n == b.n && a.mode == b.mode && lacp(&a.mode) => modes.push(a.mode.clone()),
            _ => return false,
        }
    }
    !(modes[0] == "passive" && modes[1] == "passive") && ok(st, "PC-Bo", "192.168.1.70")
}
pub(crate) fn ck_c2n1(st: &mut State) -> bool { st.dev("SW1").cfg().lldp && st.dev("SW2").cfg().lldp }
// Modul 3
pub(crate) fn ck_c3k1(st: &mut State) -> bool {
    let d = sim::get(st);
    [10u16, 20, 30, 99].iter().all(|v| d.stp.get("SW1").and_then(|m| m.get(v)).map(|i| i.is_root).unwrap_or(false))
}
pub(crate) fn ck_c3k2(st: &mut State) -> bool { !errdis(st, "SW1", "GigabitEthernet0/6") && ok(st, "PC-Karim", "192.168.1.10") }
pub(crate) fn ck_c3n1(st: &mut State) -> bool {
    ["SW1", "SW2"].iter().all(|s| gx(st.dev(s).cfg(), "stp mode").map(|v| v.ends_with("rapid-pvst")).unwrap_or(false))
}
pub(crate) fn ck_c3n2(st: &mut State) -> bool {
    let c = st.dev("SW1").cfg();
    if c.x.contains_key("stp bpdu default") {
        return true;
    }
    (5..=12).all(|n| {
        c.ifaces.get(&format!("GigabitEthernet0/{}", n)).and_then(|i| i.x.get("bpduguard")).map(|v| v.ends_with("enable")).unwrap_or(false)
    })
}
// Modul 4
pub(crate) fn ck_c4k1(st: &mut State) -> bool { ok(st, "PC-Lager", "192.168.1.10") && ok(st, "PC-Anna", "192.168.2.1") }
pub(crate) fn ck_c4k2(st: &mut State) -> bool { ok(st, "PC-Anna", "198.51.100.80") && ok(st, "PC-Lager", "198.51.100.80") }
pub(crate) fn ck_c4n1(st: &mut State) -> bool { misc(st, "routeLooked") }
// Modul 5
pub(crate) fn ck_c5k1(st: &mut State) -> bool {
    ospf_nbrs(st, "R1").iter().any(|r| r == "3.3.3.3") && ospf_nbrs(st, "RM").iter().any(|r| r == "1.1.1.1")
}
pub(crate) fn ck_c5k2(st: &mut State) -> bool {
    let n = ospf_nbrs(st, "RB");
    n.iter().any(|r| r == "1.1.1.1") && n.iter().any(|r| r == "3.3.3.3")
}
pub(crate) fn ck_c5n1(st: &mut State) -> bool {
    let p2p = |st: &State, d: &str| {
        st.dev(d).cfg().ifaces.get("GigabitEthernet0/2").and_then(|i| i.x.get("ospf net")).map(|v| v.ends_with("point-to-point")).unwrap_or(false)
    };
    p2p(st, "R1") && p2p(st, "RB") && ospf_nbrs(st, "R1").iter().any(|r| r == "2.2.2.2")
}
// Modul 6
pub(crate) fn ck_c6k1(st: &mut State) -> bool {
    let nat_ok = st.dev("R1").cfg().nat.statics.iter().any(|s| s.global == "203.0.113.11" && s.local == "192.168.1.10")
        && !st.dev("R1").cfg().nat.statics.iter().any(|s| s.global == "203.0.113.11" && s.local != "192.168.1.10");
    nat_ok && sim::ping(st, "Tekniker", "203.0.113.11", PingOpts::default()).at.as_deref() == Some("Filserver")
}
pub(crate) fn ck_c6k2(st: &mut State) -> bool {
    let c = st.dev("SW1").cfg();
    let lvl = gx(c, "logging trap").and_then(|v| v.split_whitespace().last()).and_then(syslog_level).unwrap_or(6);
    c.logging.hosts.iter().any(|h| h == "192.168.1.17") && lvl >= 5
}
pub(crate) fn ck_c6n1(st: &mut State) -> bool { misc(st, "tftp:R1:192.168.1.10") }
// Modul 7
pub(crate) fn ck_c7k1(st: &mut State) -> bool {
    let ps = st.dev("SW1").cfg().ifaces["GigabitEthernet0/6"].port_sec.as_ref().map(|p| !p.disabled).unwrap_or(false);
    ps && !errdis(st, "SW1", "GigabitEthernet0/6") && ok(st, "PC-Karim", "192.168.1.10")
}
pub(crate) fn ck_c7k2(st: &mut State) -> bool {
    let ac = match st.dev("R1").cfg().lines.vty0_4.access_class.clone() {
        Some(a) if st.dev("R1").cfg().acls.contains_key(&a) => a,
        _ => return false,
    };
    let p = |src: &str| AclPkt { src: src.into(), dst: "192.168.1.193".into(), proto: "tcp".into(), dport: Some(22) };
    sim::acl_eval(st, "R1", &ac, &p("192.168.1.200"))
        && !sim::acl_eval(st, "R1", &ac, &p("192.168.1.20"))
        && !sim::acl_eval(st, "R1", &ac, &p("192.168.1.140"))
}
pub(crate) fn ck_c7k3(st: &mut State) -> bool {
    match &st.dev("R1").cfg().snmp {
        None => false,
        Some(s) => {
            !s.communities.keys().any(|k| k == "public" || k == "private")
                && !s.communities.values().any(|v| v == "RW")
                && s.communities.get("nordvik-ro").map(|v| v == "RO").unwrap_or(false)
        }
    }
}
pub(crate) fn ck_c7n1(st: &mut State) -> bool {
    let c = st.dev("SW1").cfg();
    let vl = gx(c, "dhcp snooping vlan").map(vlans_in).unwrap_or_default();
    c.x.contains_key("dhcp snooping")
        && [10, 20, 30].iter().all(|v| vl.contains(v))
        && c.ifaces.get("GigabitEthernet0/1").map(|i| i.x.contains_key("snoop trust")).unwrap_or(false)
}
// Modul 8
pub(crate) fn ck_c8k1(st: &mut State) -> bool {
    let w = st.dev("WLC").wlc.as_ref().unwrap();
    w.wlans.get(&2).map(|x| x.enabled && (x.security == "WPA2-PSK" || x.security == "WPA3-SAE")).unwrap_or(false)
}
pub(crate) fn ck_c8k2(st: &mut State) -> bool {
    let v: Vec<u32> = st.dev("WLC").wlc.as_ref().unwrap().ap_channels.values().copied().collect();
    v.len() == 3 && v.iter().all(|c| [1, 6, 11].contains(c)) && v[0] != v[1] && v[1] != v[2] && v[0] != v[2]
}
pub(crate) fn ck_c8n1(st: &mut State) -> bool { misc(st, "apiAps") }

// ------------------------------------------------------------------ Titta-flaggor

/// Flaggor som spelet sätter när spelaren kör ett kommando (CCNA-modulernas "titta"-uppgifter)
pub(super) fn look_flags(dev: &str, line: &str) -> Vec<&'static str> {
    let mut v = vec![];
    if dev == "R1" && show_match(line, &[(true, "ip"), (false, "ro")], false) {
        v.push("routeLooked");
    }
    let low = line.to_lowercase();
    let web = ["curl", "wget", "invoke-webrequest", "invoke-restmethod", "iwr", "irm"].iter().any(|c| first_word(&low) == *c || low.trim_start().starts_with(c));
    if web && low.contains("/api/v1/aps") {
        v.push("apiAps");
    }
    v
}

// ------------------------------------------------------------------ Svårt läge

fn mk(on: &[&'static str], cmds: &[&'static str], text: &'static str) -> Verify {
    Verify { on: on.to_vec(), cmds: cmds.to_vec(), text }
}
pub(super) fn verify_table(id: &str) -> Option<Verify> {
    Some(match id {
        "c1k1" => mk(&["SW1"], &["show interfaces gi0/12", "show interfaces status"], "Bevisa: visa Saras port (show interfaces gi0/12) på SW1 – Full-duplex, 1000Mb/s."),
        "c1k2" => mk(&["PC-Lisa"], &["ping 198.51.100.80", "ping 203.0.113.1", "ipconfig"], "Bevisa: pinga internet (198.51.100.80) från Lisas dator."),
        "c1n1" => mk(&["R1"], &["show ipv6 interface brief", "show running-config interface gi0/0.10"], "Bevisa: visa IPv6-adresserna på R1 (show ipv6 interface brief)."),
        "c2k1" => mk(&["PC-Bo", "PC-Maja", "SW2"], &["ping 192.168.1.70", "ping ekonomi", "show vlan brief"], "Bevisa: pinga ekonomisystemet (192.168.1.70) från Bos dator, eller visa show vlan brief på SW2."),
        "c2k2" => mk(&["SW1", "SW2"], &["show etherchannel summary"], "Bevisa: visa show etherchannel summary – portarna ska ha flaggan P."),
        "c2n1" => mk(&["SW1", "SW2"], &["show lldp neighbors"], "Bevisa: visa LLDP-grannarna (show lldp neighbors) på SW1 eller SW2."),
        "c3k1" => mk(&["SW1", "SW2"], &["show spanning-tree vlan 10", "show spanning-tree root", "show spanning-tree"], "Bevisa: visa show spanning-tree vlan 10 – SW1 ska vara rot."),
        "c3k2" => mk(&["PC-Karim", "SW1"], &["ping 192.168.1.10", "ping filserver", "show interfaces status"], "Bevisa: pinga filservern från Karims dator."),
        "c3n1" => mk(&["SW1", "SW2"], &["show spanning-tree", "show spanning-tree summary"], "Bevisa: visa show spanning-tree – protokollet ska vara rstp."),
        "c3n2" => mk(&["SW1"], &["show spanning-tree summary", "show running-config | include bpduguard", "show running-config"], "Bevisa: visa show spanning-tree summary (BPDU Guard Default) eller konfigurationen på SW1."),
        "c4k1" => mk(&["PC-Lager", "R1"], &["ping 192.168.1.10", "ping filserver", "show ip route 192.168.2.20", "show ip route"], "Bevisa: pinga filservern från lagerdatorn, eller visa show ip route 192.168.2.20 på R1."),
        "c4k2" => mk(&["PC-Anna", "PC-Lager", "R1"], &["ping 198.51.100.80", "traceroute 198.51.100.80", "tracert 198.51.100.80", "show ip route"], "Bevisa: pinga internet (198.51.100.80) från en dator, eller visa routingtabellen på R1."),
        "c4n1" => mk(&["R1"], &["show ip route"], "Bevisa: visa routingtabellen (show ip route) på R1."),
        "c5k1" => mk(&["R1", "RM"], &["show ip ospf neighbor"], "Bevisa: visa OSPF-grannarna (show ip ospf neighbor) på R1 – 3.3.3.3 ska vara FULL."),
        "c5k2" => mk(&["RB"], &["show ip ospf neighbor", "show ip protocols"], "Bevisa: visa OSPF-grannarna (show ip ospf neighbor) på RB."),
        "c5n1" => mk(&["R1", "RB"], &["show ip ospf neighbor", "show ip ospf interface gi0/2"], "Bevisa: visa show ip ospf interface gi0/2 (POINT_TO_POINT) eller grannarna på R1 eller RB."),
        "c6k1" => mk(&["R1", "Tekniker"], &["show ip nat translations", "ping 203.0.113.11", "curl http://203.0.113.11"], "Bevisa: visa NAT-översättningarna (show ip nat translations) på R1, eller pinga 203.0.113.11 från laptopen och se att filservern svarar."),
        "c6k2" => mk(&["SW1"], &["show logging"], "Bevisa: visa show logging på SW1 – loggservern och nivån ska synas."),
        "c6n1" => mk(&["R1"], &["copy running-config tftp", "show running-config", "show startup-config"], "Bevisa: kör kopieringen till TFTP-servern igen eller visa konfigurationen på R1."),
        "c7k1" => mk(&["PC-Karim", "SW1"], &["ping 192.168.1.10", "ping filserver", "show port-security interface gi0/6", "show interfaces status"], "Bevisa: pinga filservern från Karims dator, eller visa show port-security interface gi0/6."),
        "c7k2" => mk(&["Tekniker", "R1"], &["ssh drift@192.168.1.193", "ssh -l drift 192.168.1.193", "ssh 192.168.1.193", "show access-lists VTY-SKYDD"], "Bevisa: logga in med ssh drift@192.168.1.193 från laptopen."),
        "c7k3" => mk(&["R1"], &["show running-config | include snmp", "show snmp community", "show running-config"], "Bevisa: visa SNMP-konfigurationen (show running-config | include snmp) på R1."),
        "c7n1" => mk(&["SW1"], &["show ip dhcp snooping"], "Bevisa: visa show ip dhcp snooping på SW1."),
        "c8k1" => mk(&["WLC"], &["show wlan summary", "show wlan 2"], "Bevisa: visa show wlan summary på controllern."),
        "c8k2" => mk(&["WLC"], &["show advanced 802.11b summary", "show ap summary"], "Bevisa: visa kanalerna (show advanced 802.11b summary) på controllern."),
        "c8n1" => mk(&["Tekniker", "PC-Anna"], &["curl"], "Bevisa: hämta http://192.168.1.196/api/v1/aps med curl från laptopen."),
        _ => return None,
    })
}

// ------------------------------------------------------------------ Modulerna

const INTRO_CCNA: &str = "CCNA-läget tränar det som Ciscos certifiering CCNA 200-301 kräver – på Nordviks riktiga nät.";

macro_rules! q {
    ($q:expr, [$($o:expr),+], $e:expr) => {
        Quiz { q: $q, options: vec![$($o),+], answer: 0, explain: $e }
    };
}
macro_rules! rep {
    ([$($c:expr),+], [$($f:expr),+]) => {
        Some(Report { cause: vec![$($c),+], fix: vec![$($f),+] })
    };
}

pub fn weeks() -> Vec<Week> {
    let _ = INTRO_CCNA;
    vec![
    // ---------------------------------------------------------------- Modul 1
    Week {
        week: 201, build: None, kurs: 3,
        title: "Nätverksgrunder och IPv6", chapter: "C1", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 1 av 8. Här tränar du det som Ciscos certifiering CCNA 200-301 kräver – på Nordviks riktiga nät.\n\nModul 1 motsvarar område 1 i provet, Network Fundamentals (20 %): gränssnitt och kablar, klientens IP-inställningar och IPv6. Frågesporten efteråt går igenom teorin: TCP och UDP, privata adresser, subnät, IPv6-adresstyper, EUI-64, topologier, fiber och virtualisering.",
        learn: vec!["Duplex och hastighet: CRC och late collisions", "Klientens IP-inställningar: adress, mask, gateway och DNS", "IPv6: ipv6 unicast-routing, /64 och link-local", "TCP mot UDP och privata adresser (RFC 1918)", "Topologier: två/tre lager, spine-leaf, WAN, SOHO och moln"],
        chips: vec!["show interfaces gi0/12", "show interfaces status", "show ipv6 interface brief"],
        tasks: vec![
            Task { id: "c1k1", krabba: true, build: false, npc: "Sara", target: "SW1",
                title: "Saras dator går långsamt",
                ticket: "Sara: \"Allt går så segt i dag. Filer tar evigheter att öppna och ibland hackar ljudet i videosamtalen.\"",
                hints: vec!["Seg men fungerande trafik: titta på felräknarna. show interfaces gi0/12 på SW-Nordvik-1 – det är Saras port.", "Står porten låst på 100 Mbit/s och full duplex? Då förhandlar Saras dator (som står på auto) inte med switchen och väljer halv duplex. Den ena sidan räknar CRC-fel, den andra late collisions.", "interface gi0/12 → speed auto → duplex auto. Kontrollera med show interfaces gi0/12 att det står Full-duplex, 1000Mb/s."],
                check: ck_c1k1,
                report: rep!(["Duplex mismatch: switchporten var låst till 100/full medan datorn stod på auto och valde halv duplex", "Saras nätverkskabel var trasig", "Porten låg i fel VLAN", "Filservern var överbelastad"], ["speed auto och duplex auto på Gi0/12", "Byta kabel", "Byta VLAN på porten", "Starta om filservern"]),
                flag: None },
            Task { id: "c1k2", krabba: true, build: false, npc: "Lisa", target: "PC-Lisa",
                title: "Lisa når filservern men inte internet",
                ticket: "Lisa: \"Filservern fungerar, men ingen webbsida går att öppna. Någon från 'IT' var här i går och 'fixade' min dator…\"",
                hints: vec!["Når du det lokala nätet men inget utanför? Kontrollera standardgatewayen. Kör ipconfig /all hos Lisa.", "Lisa sitter i kontorsnätet 192.168.1.0/26 (VLAN 10). Gatewayen måste ligga i samma subnät, alltså 192.168.1.1. 192.168.1.65 är ekonominätets gateway.", "Öppna Lisas nätverksinställningar (E på datorn → Nätverksinställningar) och välj DHCP, eller sätt gateway 192.168.1.1."],
                check: ck_c1k2,
                report: rep!(["Statisk adress med fel standardgateway – 192.168.1.65 ligger i ett annat subnät", "Fel nätmask", "DNS-server saknades", "Internetlänken var nere"], ["Sätta gateway 192.168.1.1 (eller DHCP) på Lisas dator", "Ändra nätmasken till /24", "Lägga in en DNS-server", "Ringa operatören"]),
                flag: None },
            Task { id: "c1n1", krabba: false, build: true, npc: "Omar", target: "R1",
                title: "Slå på IPv6 i kontorsnätet",
                ticket: "Omar: \"Vi ska köra dual stack. Slå på IPv6-routing på R-Nordvik-1 och ge kontorets sub-interface Gi0/0.10 adressen 2001:db8:acad:10::1/64.\"",
                hints: vec!["En Cisco-router routar inte IPv6 förrän du slår på det: configure terminal → ipv6 unicast-routing.", "interface gi0/0.10 → ipv6 address 2001:db8:acad:10::1/64. Ett /64 är standardstorleken för ett IPv6-LAN: 64 bitar nät och 64 bitar interface-ID.", "Kontrollera med show ipv6 interface brief. Interfacet får också en link-local-adress (FE80::) automatiskt."],
                check: ck_c1n1, report: None, flag: None },
        ],
        quiz: vec![
            q!("Vilket transportprotokoll använder en trevägshandskakning (SYN, SYN-ACK, ACK)?", ["TCP", "UDP", "ICMP"], "TCP är förbindelseorienterat och garanterar leverans med sekvensnummer och kvittenser. UDP skickar utan handskakning – snabbare men utan garantier (DNS-frågor, röst, TFTP)."),
            q!("Vilket nät är privat enligt RFC 1918?", ["172.20.0.0/16", "172.32.0.0/16", "192.169.0.0/16"], "Privata nät: 10.0.0.0/8, 172.16.0.0/12 (172.16–172.31) och 192.168.0.0/16. De routas inte på internet och kräver NAT ut."),
            q!("Hur många användbara värdadresser finns i ett /27?", ["30", "32", "62"], "/27 lämnar 5 värdbitar: 2^5 = 32 adresser, minus nätadress och broadcast = 30. Blocksteg 256 − 224 = 32."),
            q!("Vilket prefix har IPv6 link-local-adresser?", ["FE80::/10", "FF00::/8", "2000::/3"], "Link-local skapas automatiskt på varje IPv6-interface och används bara på den lokala länken, t.ex. som nästa hopp och för grannupptäckt. FF00::/8 är multicast och 2000::/3 global unicast."),
            q!("Vilken IPv6-adresstyp motsvarar privata IPv4-adresser?", ["Unique local (FC00::/7)", "Link-local (FE80::/10)", "Anycast"], "Unique local (i praktiken FD00::/8) routas inom organisationen men inte på internet. Anycast är samma adress på flera enheter – paketet går till den närmaste."),
            q!("Hur bildar modifierad EUI-64 ett interface-ID av en MAC-adress?", ["Delar MAC:en på mitten, stoppar in FFFE och vänder den sjunde biten", "Upprepar MAC-adressen två gånger", "Slumpar fram 64 bitar"], "Exempel: 0011.2233.4455 → 0211:22FF:FE33:4455. FFFE hamnar i mitten och U/L-biten (bit 7) vänds."),
            q!("Hur kortas 2001:0db8:0000:0000:0000:0000:0000:0001 korrekt?", ["2001:db8::1", "2001:db8:0:1", "2001:db8::0::1"], "Inledande nollor i varje grupp får strykas, och en enda sammanhängande serie nollgrupper får ersättas med ::."),
            q!("Vad är typiskt för en spine-leaf-topologi?", ["Varje leaf-switch är kopplad till varje spine-switch", "Alla switchar sitter i en lång kedja", "Varje server har en egen router"], "Spine-leaf ger lika många hopp mellan alla servrar och är vanlig i datacenter. Campusnät byggs ofta i två lager (collapsed core) eller tre lager: access, distribution och core."),
            q!("Vilken fibertyp används för flera kilometers avstånd?", ["Singlemode", "Multimode", "Cat 6"], "Singlemode har tunn kärna och laser – lång räckvidd. Multimode har tjockare kärna och är billigare men når bara hundratals meter. Koppar (Cat 5e/6) når 100 m."),
            q!("Vad gör en switch med en ram till en okänd MAC-adress?", ["Floodar den på alla portar i VLAN:et utom den den kom in på", "Kastar ramen", "Skickar den till routern"], "Switchen lär sig avsändarens MAC-adress per port och floodar ramar till okända mål (unknown unicast). Poster i MAC-tabellen åldras ut efter 300 sekunder som standard."),
            q!("Vad är en hypervisor av typ 1?", ["En hypervisor som körs direkt på hårdvaran", "Ett program som körs ovanpå Windows", "En virtuell switch"], "Typ 1 (ESXi, Hyper-V, KVM) körs direkt på hårdvaran, typ 2 (VirtualBox) ovanpå ett operativsystem. Virtuella maskiner kopplas ihop med virtuella switchar; containrar delar värdens kärna."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 2
    Week {
        week: 202, build: None, kurs: 3,
        title: "VLAN och EtherChannel", chapter: "C2", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 2 av 8: område 2 i provet, Network Access (20 %).\n\nDen här modulen handlar om VLAN-databasen, EtherChannel med LACP och grannprotokollen CDP och LLDP. Frågesporten tar också upp trunkar, native VLAN, DTP, router-on-a-stick och voice VLAN.",
        learn: vec!["show vlan brief och VLAN-databasen", "EtherChannel: LACP active/passive, PAgP och on", "show etherchannel summary", "CDP (Cisco) och LLDP (IEEE 802.1AB)", "802.1Q, native VLAN och DTP"],
        chips: vec!["show vlan brief", "show etherchannel summary", "show lldp neighbors"],
        tasks: vec![
            Task { id: "c2k1", krabba: true, build: false, npc: "Bo", target: "SW2",
                title: "Ingen når ekonomisystemet",
                ticket: "Bo: \"Ekonomisystemet svarar inte, varken för mig eller Maja. Internet fungerar som vanligt.\"",
                hints: vec!["Ekonomisystemet sitter på SW-Nordvik-2 Gi0/8 i VLAN 20. Kör show interfaces status där – vad står det för porten?", "show vlan brief på SW-Nordvik-2. Finns VLAN 20 kvar? En port i ett VLAN som inte finns blir inaktiv, och trunken slutar bära VLAN:et.", "configure terminal → vlan 20 → name EKONOMI. Porten blir aktiv så fort VLAN:et finns igen."],
                check: ck_c2k1,
                report: rep!(["VLAN 20 var raderat ur SW-Nordvik-2:s VLAN-databas", "VLAN 20 saknades i trunkens allowed-lista", "Ekonomisystemet var avstängt", "Routern saknade sub-interface för VLAN 20"], ["Skapa VLAN 20 (name EKONOMI) på SW-Nordvik-2", "switchport trunk allowed vlan add 20", "Starta ekonomisystemet", "Skapa sub-interface Gi0/0.20"]),
                flag: None },
            Task { id: "c2k2", krabba: true, build: false, npc: "Omar", target: "SW2",
                title: "EtherChannel mellan switcharna kommer inte upp",
                ticket: "Omar: \"Jag började bunta ihop trunkarna Gi0/24–25 till en EtherChannel med LACP i går men blev avbruten. Loggen klagar och show etherchannel summary ser konstig ut.\"",
                hints: vec!["Kör show etherchannel summary på båda switcharna. P = buntad, I = stand-alone, D = nere, s = suspenderad.", "Jämför läget med show running-config interface gi0/24 på båda. LACP pratar bara med LACP: active–active eller active–passive. Läget on förhandlar inget och fungerar bara mot on.", "På SW-Nordvik-2: interface range gi0/24 - 25 → channel-group 1 mode active. Portarna ska få flaggan P."],
                check: ck_c2k2,
                report: rep!(["Ena sidan stod i LACP active och andra i mode on, som inte förhandlar LACP", "Trunkarna hade olika native VLAN", "Kabeln mellan switcharna var trasig", "Spanning tree blockerade Port-channel1"], ["channel-group 1 mode active (eller passive) på SW-Nordvik-2:s portar", "Sätta native vlan 999", "Byta kabel", "Stänga av spanning tree"]),
                flag: None },
            Task { id: "c2n1", krabba: false, build: true, npc: "Omar", target: "SW1",
                title: "Slå på LLDP på switcharna",
                ticket: "Omar: \"Den nya övervakningen förstår inte Ciscos CDP, bara standarden LLDP. Slå på LLDP på båda switcharna.\"",
                hints: vec!["CDP är Ciscos eget grannprotokoll och är på som standard. LLDP (IEEE 802.1AB) är standarden och är av som standard på Cisco.", "configure terminal → lldp run – på både SW-Nordvik-1 och SW-Nordvik-2.", "Kontrollera med show lldp neighbors och jämför med show cdp neighbors."],
                check: ck_c2n1, report: None, flag: None },
        ],
        quiz: vec![
            q!("Vilket VLAN är native VLAN på en trunk om inget annat konfigureras?", ["VLAN 1", "VLAN 99", "VLAN 1002"], "Native VLAN skickas otaggat över trunken. Det måste vara samma i båda ändar; Nordvik använder 999 för att inte blanda in VLAN 1."),
            q!("Hur stor är 802.1Q-taggen?", ["4 byte", "2 byte", "8 byte"], "Taggen innehåller TPID 0x8100, prioritet (PCP), DEI och 12 bitar VLAN-ID, vilket ger VLAN 1–4094."),
            q!("Vilka två LACP-lägen bildar en EtherChannel tillsammans?", ["active och passive", "passive och passive", "on och active"], "Active förhandlar, passive svarar bara – två passive gör ingenting. PAgP (Cisco) använder desirable och auto. Läget on förhandlar inte alls och kräver on i andra änden."),
            q!("Vad måste vara lika på alla portar i en EtherChannel?", ["Hastighet, duplex och VLAN-/trunkinställningar", "Portbeskrivningen", "MAC-adressen"], "Medlemsportarna måste ha samma hastighet, duplex, switchport-läge, access-VLAN eller allowed/native-VLAN. Annars suspenderas porten."),
            q!("Vilket grannprotokoll är en öppen IEEE-standard?", ["LLDP", "CDP", "DTP"], "LLDP är IEEE 802.1AB. CDP och DTP är Ciscos egna. show cdp neighbors detail visar grannens IP-adress, modell och IOS-version."),
            q!("Vad gör switchport nonegotiate?", ["Stänger av DTP på porten", "Stänger av CDP", "Låser hastigheten"], "DTP förhandlar trunkläge (dynamic auto/desirable). Säkrast är att sätta access eller trunk manuellt och stänga av DTP."),
            q!("Vad behövs för routing mellan VLAN med router-on-a-stick?", ["Ett sub-interface per VLAN med encapsulation dot1Q och en trunk mot switchen", "En accessport per VLAN på routern", "Spanning tree på routern"], "Varje sub-interface, t.ex. Gi0/0.10, får encapsulation dot1Q 10 och gatewayadressen. Alternativet är SVI:er (interface vlan 10) på en lager 3-switch med ip routing."),
            q!("Vad händer med en port vars access-VLAN inte finns i VLAN-databasen?", ["Porten blir inaktiv och skickar ingen trafik", "Porten hamnar i VLAN 1", "Switchen skapar VLAN:et automatiskt"], "Skapa VLAN:et med vlan <id> så blir porten aktiv igen. show vlan brief visar vilka VLAN som finns och vilka portar som ligger i dem."),
            q!("Vad är en voice VLAN?", ["Ett extra VLAN på en accessport som IP-telefonen taggar sin trafik i", "Ett VLAN för trådlösa gäster", "Native VLAN på trunkar"], "switchport voice vlan 150 låter telefonen tagga rösttrafiken medan datorn bakom telefonen går otaggad i access-VLAN:et."),
            q!("Vilket kommando visar vilka VLAN som faktiskt går över en trunk?", ["show interfaces trunk", "show vlan brief", "show cdp neighbors"], "show interfaces trunk visar läge, inkapsling, native VLAN, tillåtna VLAN och vilka som är aktiva och inte blockerade av spanning tree."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 3
    Week {
        week: 203, build: None, kurs: 3,
        title: "Spanning tree", chapter: "C3", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 3 av 8: spanning tree ur område 2 i provet.\n\nDu tränar rotbryggeval, BPDU guard och err-disabled, Rapid PVST+ och PortFast. Frågesporten går igenom bridge ID, rotport, porttillstånd, kostnader, root guard, loop guard och BPDU filter.",
        learn: vec!["Rotbrygga: lägst bridge ID (prioritet + VLAN, sedan MAC)", "Rotport, designated port och alternate port", "PortFast och BPDU guard, err-disabled", "spanning-tree mode rapid-pvst", "Root guard, loop guard och BPDU filter"],
        chips: vec!["show spanning-tree vlan 10", "show interfaces status", "show spanning-tree summary"],
        tasks: vec![
            Task { id: "c3k1", krabba: true, build: false, npc: "Omar", target: "SW2",
                title: "Rotbryggan har flyttat sig",
                ticket: "Omar: \"Övervakningen säger att rotbryggan för VLAN 10 har flyttat från SW-Nordvik-1 till SW-Nordvik-2. Den ska vara SW-Nordvik-1, som sitter närmast routern.\"",
                hints: vec!["show spanning-tree vlan 10 på båda switcharna. Rotbryggan är den med lägst bridge ID: prioritet (+ VLAN-numret) och sedan MAC-adress.", "SW-Nordvik-1 har prioritet 24576 för VLAN 10. Vad har SW-Nordvik-2? Leta efter spanning-tree vlan 10 priority i show running-config.", "På SW-Nordvik-2: no spanning-tree vlan 10 priority 4096 (tillbaka till 32768). show spanning-tree vlan 10 på SW-Nordvik-1 ska säga This bridge is the root."],
                check: ck_c3k1,
                report: rep!(["SW-Nordvik-2 hade fått prioritet 4096 för VLAN 10 och vann rotvalet", "Länken mellan switcharna var trasig", "Spanning tree var avstängt för VLAN 10", "SW-Nordvik-1 hade startat om"], ["Ta bort prioriteten på SW-Nordvik-2 (eller sänka SW-Nordvik-1:s)", "Byta kabel", "Slå på spanning tree", "Starta om båda switcharna"]),
                flag: None },
            Task { id: "c3k2", krabba: true, build: false, npc: "Karim", target: "SW1",
                title: "Karims port är err-disabled",
                ticket: "Karim: \"Jag kopplade in en liten switch vid skrivbordet i går för att testa en grej. Nu har jag tagit bort den, men nätet kommer inte tillbaka!\"",
                hints: vec!["show interfaces status på SW-Nordvik-1. Står Karims port (Gi0/6) som err-disabled?", "show logging: BPDU guard stängde porten när Karims switch skickade BPDU:er. BPDU guard hör hemma på PortFast-portar – där ska bara datorer sitta.", "Switchen är borta, så porten kan öppnas igen: interface gi0/6 → shutdown → no shutdown. Låt BPDU guard sitta kvar."],
                check: ck_c3k2,
                report: rep!(["BPDU guard satte porten i err-disabled när en switch skickade BPDU:er", "Port security hade fel MAC-adress", "Karims kabel var trasig", "DHCP-poolen var full"], ["shutdown och no shutdown på Gi0/6 när switchen var borttagen", "Stänga av BPDU guard", "Byta kabel", "Göra poolen större"]),
                flag: None },
            Task { id: "c3n1", krabba: false, build: true, npc: "Omar", target: "SW1",
                title: "Byt till Rapid PVST+",
                ticket: "Omar: \"Vanlig PVST+ tar 30–50 sekunder på sig när något ändras. Byt till Rapid PVST+ (802.1w) på båda switcharna.\"",
                hints: vec!["Rapid PVST+ konvergerar på någon sekund med proposal/agreement i stället för att vänta ut timers.", "configure terminal → spanning-tree mode rapid-pvst – på SW-Nordvik-1 och SW-Nordvik-2.", "Kontrollera med show spanning-tree: det ska stå Spanning tree enabled protocol rstp."],
                check: ck_c3n1, report: None, flag: None },
            Task { id: "c3n2", krabba: false, build: true, npc: "Omar", target: "SW1",
                title: "BPDU guard på alla PortFast-portar",
                ticket: "Omar: \"Efter Karims lilla switch vill jag ha BPDU guard på alla accessportar med PortFast på SW-Nordvik-1.\"",
                hints: vec!["Accessportarna har redan PortFast. BPDU guard stänger porten så fort en BPDU kommer in.", "Enklast globalt: configure terminal → spanning-tree portfast bpduguard default. Det gäller alla portar med PortFast.", "Eller per port: interface range gi0/5 - 12 → spanning-tree bpduguard enable."],
                check: ck_c3n2, report: None, flag: None },
        ],
        quiz: vec![
            q!("Hur väljs rotbryggan?", ["Lägst bridge ID: prioritet först, sedan MAC-adress", "Högst bridge ID", "Switchen med flest portar"], "Alla switchar skickar BPDU:er med sitt bridge ID. Den med lägst värde blir rot, och alla dess portar blir designated."),
            q!("Vilken är standardprioriteten i spanning tree?", ["32768", "4096", "0"], "Prioriteten sätts i steg om 4096 (0–61440). PVST+ lägger till VLAN-numret (extended system ID), så VLAN 10 ger 32778."),
            q!("Vilken port blir rotport på en switch som inte är rot?", ["Porten med lägst kostnad till rotbryggan", "Porten med högst nummer", "Porten med flest MAC-adresser"], "Vid lika kostnad avgör grannens bridge ID och sedan portprioritet och portnummer. Varje segment har en designated port; övriga portar blir alternate och blockerar."),
            q!("Vilka porttillstånd finns i RSTP (802.1w)?", ["Discarding, learning och forwarding", "Blocking, listening, learning, forwarding och disabled", "Up och down"], "Klassisk STP har blocking, listening, learning och forwarding (30 sekunder till forwarding med standardtimers). RSTP slår ihop blocking, listening och disabled till discarding."),
            q!("Vad gör PortFast?", ["Porten går direkt till forwarding utan att vänta på STP-timers", "Porten får högre hastighet", "Porten blir en trunk"], "PortFast är för datorer, skrivare och servrar (edge-portar), aldrig för portar mot andra switchar."),
            q!("Vad gör BPDU guard när en BPDU kommer in på porten?", ["Sätter porten i err-disabled", "Gör porten till rotport", "Ignorerar BPDU:n"], "Porten öppnas med shutdown/no shutdown eller automatiskt med errdisable recovery cause bpduguard. BPDU filter slutar i stället skicka och ta emot BPDU:er – farligt om det blir en loop."),
            q!("Vad skyddar root guard mot?", ["Att en switch på fel ställe blir rotbrygga", "Att porten hamnar i fel VLAN", "Broadcaststormar från datorer"], "Får porten en bättre BPDU hamnar den i root-inconsistent. Loop guard skyddar mot att en blockerad port börjar vidarebefordra när BPDU:erna slutar komma, t.ex. vid en enkelriktad länk."),
            q!("Vilken kostnad har en 1 Gbit/s-länk i klassisk (short) STP?", ["4", "19", "100"], "Short-metoden: 10 Mbit/s = 100, 100 Mbit/s = 19, 1 Gbit/s = 4, 10 Gbit/s = 2."),
            q!("Vad gör spanning-tree vlan 10 root primary?", ["Sätter en prioritet som gör switchen till rot för VLAN 10", "Stänger av VLAN 10", "Gör switchen till VTP-server"], "root primary sätter 24576 eller lägre än nuvarande rot. root secondary sätter 28672 för en reservrot."),
            q!("Hur många STP-instanser kör PVST+?", ["En per VLAN", "En för hela switchen", "En per port"], "Med en instans per VLAN kan olika VLAN ha olika rot och dela på länkarna. MST grupperar flera VLAN i samma instans."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 4
    Week {
        week: 204, build: None, kurs: 3,
        title: "Statiska rutter", chapter: "C4", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 4 av 8: område 3 i provet, IP Connectivity (25 %).\n\nHur väljer routern väg? Först längsta prefix, sedan lägst administrativt avstånd, sist lägst metrik. Den här modulen har ett fel av varje sort, plus en genomgång av routingtabellen. Frågesporten tar också upp flytande statiska rutter, värdrutter och IPv6-rutter.",
        learn: vec!["Längsta prefixmatchning", "Administrativt avstånd: 0, 1, 20, 90, 110, 120", "Flytande statiska rutter (AD 250)", "Koderna C, L, S och S* i show ip route", "IPv6-rutter: ipv6 route ::/0"],
        chips: vec!["show ip route", "show running-config | include ip route", "traceroute 198.51.100.80"],
        tasks: vec![
            Task { id: "c4k1", krabba: true, build: false, npc: "Nils", target: "R1",
                title: "Lagret når inte filservern",
                ticket: "Nils: \"Lagerdatorn kommer inte åt filservern i Göteborg. Allt annat här i Borås fungerar.\"",
                hints: vec!["show ip route 192.168.2.20 på R-Nordvik-1. Vart skickar R1 svaren till lagret?", "Routern väljer alltid den mest specifika rutten – längsta prefix. Det finns en rutt till 192.168.2.0/24 via Borås, men finns det en /26 som pekar någon annanstans?", "configure terminal → no ip route 192.168.2.0 255.255.255.192 203.0.113.1. Då används /24-rutten via 10.0.0.2 igen."],
                check: ck_c4k1,
                report: rep!(["En mer specifik rutt (/26) pekade mot operatören och vann över /24-rutten via Borås", "Borås defaultrutt saknades", "Länken mellan kontoren var nere", "NAT översatte lagrets adresser fel"], ["Ta bort /26-rutten mot 203.0.113.1", "Lägga in en defaultrutt på RB", "Byta kabel mellan kontoren", "Ta bort NAT"]),
                flag: None },
            Task { id: "c4k2", krabba: true, build: false, npc: "Anna", target: "R1",
                title: "Internet är borta för alla",
                ticket: "Anna: \"Ingen kommer ut på internet, varken här eller i Borås. Det började efter att någon pillade på routern i natt.\"",
                hints: vec!["show ip route på R-Nordvik-1. Vilken defaultrutt (S*) används, och vilket nästa hopp har den?", "show running-config | include ip route. Det finns två defaultrutter. Siffran sist är administrativt avstånd (AD) – lägst vinner. En statisk rutt med AD 250 är en flytande reservrutt som bara används när huvudvägen försvinner.", "Rutten mot Borås (10.0.0.2) skickar trafiken i en loop, för Borås skickar tillbaka den. no ip route 0.0.0.0 0.0.0.0 10.0.0.2, och sätt gärna tillbaka ip route 0.0.0.0 0.0.0.0 203.0.113.1 utan AD."],
                check: ck_c4k2,
                report: rep!(["En defaultrutt mot Borås med AD 1 slog ut den riktiga, som fått AD 250 – trafiken loopade", "Operatörens länk var nere", "NAT var avstängt", "DNS-servern svarade inte"], ["Ta bort defaultrutten mot 10.0.0.2 (och ge den riktiga AD 1 igen)", "Ringa operatören", "Slå på NAT", "Byta DNS-server"]),
                flag: None },
            Task { id: "c4n1", krabba: false, build: false, npc: "Linnea", target: "R1",
                title: "Linnea vill förstå routingtabellen",
                ticket: "Linnea: \"Jag har börjat läsa till CCNA. Kan du visa mig routingtabellen på R-Nordvik-1 och förklara vad bokstäverna och siffrorna betyder?\"",
                hints: vec!["Kör show ip route på R-Nordvik-1 i privilegierat läge.", "Varje rad börjar med en kod: C, L, S och S*. Överst i utskriften står vad koderna betyder.", "[1/0] är [administrativt avstånd/metrik]. Skriv sedan felrapporten (F) med förklaringen."],
                check: ck_c4n1,
                report: rep!(["C = direktanslutet nät, L = routerns egen adress (/32), S = statisk rutt, S* = statisk defaultrutt", "C = Cisco, L = lokal användare, S = säker, S* = sparad", "C = kabel, L = länk nere, S = seriell, S* = stjärnnät", "C = CDP, L = LLDP, S = SNMP, S* = syslog"], ["[1/0] betyder administrativt avstånd 1 och metrik 0", "[1/0] betyder 1 paket och 0 fel", "[1/0] betyder port 1 i slot 0", "[1/0] betyder VLAN 1 och prioritet 0"]),
                flag: Some("routeLooked") },
        ],
        quiz: vec![
            q!("Tabellen har 10.0.0.0/8, 10.1.0.0/16 och 10.1.1.0/24. Vilken rutt används för 10.1.1.5?", ["10.1.1.0/24", "10.0.0.0/8", "10.1.0.0/16"], "Längsta prefixmatchning: den rutt som matchar flest bitar vinner, oavsett AD och metrik."),
            q!("Vilket administrativt avstånd har en statisk rutt som standard?", ["1", "0", "110"], "Direktanslutet 0, statisk 1, eBGP 20, EIGRP 90, OSPF 110, RIP 120, iBGP 200. Lägst AD vinner när samma prefix lärs från olika källor."),
            q!("Vad är en flytande statisk rutt?", ["En reservrutt med högre AD än huvudvägen, som bara används när huvudvägen försvinner", "En rutt som byter nästa hopp av sig själv", "En rutt till ett multicastnät"], "Exempel: ip route 0.0.0.0 0.0.0.0 198.51.100.1 250 installeras först när OSPF- eller den statiska huvudvägen försvinner."),
            q!("När jämförs metrik?", ["Mellan rutter till samma prefix från samma routingprotokoll", "Alltid före prefixlängden", "Bara för statiska rutter"], "Ordningen är: längsta prefix → lägst AD → lägst metrik. Lika metrik ger lastdelning (ECMP)."),
            q!("Vad betyder koden L i show ip route?", ["Routerns egen adress på ett interface (/32)", "En länk som är nere", "En rutt från LLDP"], "Varje adresserat interface ger två rader: C för nätet och L för den egna adressen."),
            q!("Hur skrivs en IPv6-defaultrutt?", ["ipv6 route ::/0 2001:db8:0:1::1", "ipv6 route 0.0.0.0 0.0.0.0 2001:db8:0:1::1", "ip route ::/0 2001:db8:0:1::1"], "IPv6-routing kräver ipv6 unicast-routing. Med en link-local-adress som nästa hopp måste utgående interface anges: ipv6 route ::/0 gi0/1 fe80::1."),
            q!("Vad är en värdrutt?", ["En rutt till en enda adress, mask 255.255.255.255 (/32) eller /128 i IPv6", "En rutt till hela internet", "En rutt som bara gäller servrar"], "Värdrutter används t.ex. för att styra trafiken till en enskild server eller loopback."),
            q!("Vad är gateway of last resort?", ["Den defaultrutt som används när ingen annan rutt matchar", "Den sista routern före internet", "Routern med högst router-ID"], "Utan gateway of last resort kastar routern paket som inte matchar någon rutt och skickar ICMP destination unreachable."),
            q!("Vad gör en router med ett paket som inte matchar någon rutt när det saknas defaultrutt?", ["Kastar paketet och svarar med ICMP destination unreachable", "Floodar det på alla interface", "Skickar det tillbaka till avsändaren"], "Routrar floodar aldrig – det gör bara switchar med okända MAC-adresser."),
            q!("Vad betyder [110/2] på en rutt?", ["Administrativt avstånd 110 och metrik 2", "Port 110 och VLAN 2", "110 hopp och 2 sekunder"], "110 är OSPF. Metriken är summan av kostnaderna längs vägen."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 5
    Week {
        week: 205, build: None, kurs: 3,
        title: "OSPF i en area", chapter: "C5", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 5 av 8: OSPFv2 i en area, ur område 3 i provet.\n\nGöteborg (1.1.1.1), Borås (2.2.2.2) och Mölndal (3.3.3.3) kör OSPF i area 0. Krabban har gömt ett passivt interface och ett dubblerat router-ID, och Omar vill ha point-to-point på länken mot Borås. Frågesporten går igenom timers, router-ID, DR/BDR, grannskap och kostnad.",
        learn: vec!["show ip ospf neighbor och show ip protocols", "passive-interface", "Router-ID: unikt, valordning", "Nätverkstyper: broadcast (DR/BDR) och point-to-point", "Hello 10 / dead 40 och kostnad = referens / bandbredd"],
        chips: vec!["show ip ospf neighbor", "show ip protocols", "show ip ospf interface gi0/2"],
        tasks: vec![
            Task { id: "c5k1", krabba: true, build: false, npc: "Omar", target: "R1",
                title: "OSPF-grannen i Mölndal är borta",
                ticket: "Omar: \"Övervakningen: OSPF-grannen 3.3.3.3 (R-Molndal-1) försvann från R-Nordvik-1 i natt. Mölndal fungerar via Borås, men serielänken står oanvänd.\"",
                hints: vec!["show ip ospf neighbor på R-Nordvik-1. show ip ospf interface brief visar vilka interface som kör OSPF.", "show ip protocols listar Passive Interface(s). Ett passivt interface annonserar sitt nät men skickar inga hello, så ingen granne bildas där.", "router ospf 1 → no passive-interface Serial0/0/0. Grannen 3.3.3.3 blir FULL inom några sekunder."],
                check: ck_c5k1,
                report: rep!(["Serial0/0/0 var passivt i OSPF, så inga hello skickades mot Mölndal", "Hello-timern skilde sig mellan sidorna", "Serielänken saknade clock rate", "Mölndal låg i en annan area"], ["no passive-interface Serial0/0/0 under router ospf 1", "Sätta samma hello-interval", "Sätta clock rate 64000", "Flytta Mölndal till area 0"]),
                flag: None },
            Task { id: "c5k2", krabba: true, build: false, npc: "Nils", target: "RB",
                title: "Borås routerlogg är full av varningar",
                ticket: "Nils: \"Någon från Göteborg loggade in på vår router i går. Sedan dess står loggen full av OSPF-varningar och trafiken till Göteborg tar en omväg.\"",
                hints: vec!["show logging på R-Boras-1 (RB). Står det något om DUP_RTRID?", "show ip protocols på RB visar router-ID. Göteborg har 1.1.1.1, Borås ska ha 2.2.2.2 och Mölndal 3.3.3.3. Router-ID måste vara unikt.", "router ospf 1 → router-id 2.2.2.2. På en riktig router måste du sedan köra clear ip ospf process för att det nya ID:t ska gälla."],
                check: ck_c5k2,
                report: rep!(["R-Boras-1 hade samma router-ID (1.1.1.1) som R-Nordvik-1, så de vägrade bli grannar", "Borås låg i area 1", "Länken Göteborg–Borås hade fel nätmask", "OSPF-processen hade fel nummer"], ["router-id 2.2.2.2 under router ospf 1 på RB", "Flytta Borås till area 0", "Rätta nätmasken", "Byta processnummer"]),
                flag: None },
            Task { id: "c5n1", krabba: false, build: true, npc: "Omar", target: "R1",
                title: "Point-to-point på länken Göteborg–Borås",
                ticket: "Omar: \"Gi0/2 mellan Göteborg och Borås är en ren punkt-till-punkt-länk, men OSPF väljer ändå DR och BDR där. Sätt nätverkstypen point-to-point i båda ändar.\"",
                hints: vec!["show ip ospf interface gi0/2 på R-Nordvik-1: Network Type BROADCAST, med en DR och en BDR. På Ethernet väljs DR (högst prioritet, sedan högst router-ID) för att minska antalet grannskap.", "Med bara två routrar behövs ingen DR. interface gi0/2 → ip ospf network point-to-point på både R-Nordvik-1 och RB.", "Kontrollera med show ip ospf neighbor: State FULL/  -, utan DR eller BDR."],
                check: ck_c5n1, report: None, flag: None },
        ],
        quiz: vec![
            q!("Vilka hello- och dead-timers har OSPF på Ethernet som standard?", ["10 och 40 sekunder", "30 och 120 sekunder", "5 och 15 sekunder"], "Timers måste vara lika för att grannar ska bildas. På NBMA-nät är standard 30 och 120."),
            q!("I vilken ordning väljs OSPF:s router-ID?", ["router-id-kommandot, högsta loopbackadress, högsta adress på ett aktivt interface", "Lägsta interfaceadress först", "Slumpmässigt vid start"], "Ändras router-ID medan processen kör gäller det först efter clear ip ospf process eller omstart."),
            q!("Vilken router blir DR på ett Ethernet-segment?", ["Högst OSPF-prioritet, vid lika högst router-ID", "Lägst router-ID", "Den som startade sist"], "Prioritet 0 (ip ospf priority 0) betyder aldrig DR eller BDR. Valet är inte preemptivt – en ny router tar inte över förrän DR försvinner."),
            q!("Vilken multicastadress skickar OSPF hello till?", ["224.0.0.5", "224.0.0.6", "224.0.0.10"], "224.0.0.5 når alla OSPF-routrar, 224.0.0.6 bara DR och BDR. 224.0.0.10 används av EIGRP."),
            q!("Vilket tillstånd har två OSPF-grannar när databaserna är synkade?", ["FULL", "2-WAY", "EXSTART"], "Två DROTHER-routrar på samma Ethernet stannar i 2-WAY med varandra – det är normalt. Mot DR och BDR blir de FULL."),
            q!("Hur räknas OSPF-kostnaden för ett interface?", ["Referensbandbredd delat med interfacets bandbredd", "Antal hopp", "Fördröjning plus last"], "Med standardreferensen 100 Mbit/s får allt från Fast Ethernet och uppåt kostnad 1. Höj med auto-cost reference-bandwidth (lika på alla routrar) eller sätt ip ospf cost."),
            q!("Vad måste matcha för att två routrar ska bli OSPF-grannar?", ["Area, subnät och mask, hello- och dead-timers – och router-ID måste vara olika", "Hostname och IOS-version", "Antalet interface"], "Även MTU och autentisering måste stämma. Nätverkstypen påverkar om DR väljs."),
            q!("Vilken area måste andra areor anslutas till?", ["Area 0 (backbone)", "Area 1", "Den med lägst nummer"], "I CCNA räcker en area (single area), men all trafik mellan areor går via area 0."),
            q!("Vad gör passive-interface i OSPF?", ["Nätet annonseras, men inga hello skickas och inga grannar bildas där", "Interfacet stängs av", "Interfacet får kostnad 0"], "Används mot LAN med bara datorer, så att ingen obehörig kan bli granne. passive-interface default gör alla passiva."),
            q!("Vad gör network 192.168.1.0 0.0.0.255 area 0?", ["Aktiverar OSPF på interface vars adress matchar och annonserar deras nät", "Skapar en statisk rutt", "Sätter router-ID"], "Wildcard 0.0.0.255 matchar de första tre oktetterna. Alternativet är ip ospf 1 area 0 direkt på interfacet."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 6
    Week {
        week: 206, build: None, kurs: 3,
        title: "IP-tjänster", chapter: "C6", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 6 av 8: område 4 i provet, IP Services (10 %).\n\nStatisk NAT, syslog-nivåer och säkerhetskopiering med TFTP. Frågesporten tar också upp PAT, DHCP och DHCP-relay, NTP, SNMP och QoS.",
        learn: vec!["Statisk NAT: inside local och inside global", "Syslog: logging host och nivåerna 0–7", "copy running-config tftp:", "DHCP-relay med ip helper-address", "QoS: märkning, köer, policing och shaping"],
        chips: vec!["show ip nat translations", "show logging", "copy running-config tftp:"],
        tasks: vec![
            Task { id: "c6k1", krabba: true, build: false, npc: "Omar", target: "R1",
                title: "Filserverns publika adress hamnar fel",
                ticket: "Omar: \"Revisorn utanför kontoret når inte filserverns publika adress 203.0.113.11. Hon säger att hon hamnar på någon skrivare i stället!\"",
                hints: vec!["Pinga 203.0.113.11 från laptopen – vem svarar? show ip nat translations på R-Nordvik-1: den statiska raden visar Inside global (publik) och Inside local (privat).", "Filservern är 192.168.1.10. Pekar 203.0.113.11 på rätt inside local-adress? show running-config | include ip nat.", "no ip nat inside source static 192.168.1.11 203.0.113.11 → ip nat inside source static 192.168.1.10 203.0.113.11."],
                check: ck_c6k1,
                report: rep!(["Den statiska NAT-raden översatte 203.0.113.11 till skrivaren (192.168.1.11) i stället för filservern", "PAT-raden saknade overload", "Operatören blockerade trafiken", "Filservern hade fel gateway"], ["ip nat inside source static 192.168.1.10 203.0.113.11", "Lägga till overload", "Ringa operatören", "Rätta filserverns gateway"]),
                flag: None },
            Task { id: "c6k2", krabba: true, build: false, npc: "Omar", target: "SW1",
                title: "Loggservern får inget från SW-Nordvik-1",
                ticket: "Omar: \"Loggservern (192.168.1.17) har inte fått ett enda meddelande från SW-Nordvik-1 sedan i går. Från SW-Nordvik-2 kommer allt som vanligt.\"",
                hints: vec!["show logging på SW-Nordvik-1. Står loggservern under Trap logging, och vilken nivå gäller?", "Syslog-nivåer: 0 emergencies, 1 alerts, 2 critical, 3 errors, 4 warnings, 5 notifications, 6 informational, 7 debugging. logging trap X skickar nivå X och allt allvarligare. Länk upp/ner är nivå 3–5.", "configure terminal → logging host 192.168.1.17 → logging trap informational."],
                check: ck_c6k2,
                report: rep!(["Loggservern saknades i konfigurationen och logging trap stod på critical (2)", "Loggservern var avstängd", "Switchen saknade IP-adress", "NTP var fel inställt"], ["logging host 192.168.1.17 och logging trap informational", "Starta loggservern", "Ge switchen en adress", "Ställa om NTP"]),
                flag: None },
            Task { id: "c6n1", krabba: false, build: true, npc: "Omar", target: "R1",
                title: "Säkerhetskopiera routern till TFTP",
                ticket: "Omar: \"Innan vi ändrar mer: ta en kopia av R-Nordvik-1:s running-config till TFTP-servern på filservern (192.168.1.10).\"",
                hints: vec!["TFTP använder UDP port 69 utan inloggning. Kopieringen görs i privilegierat läge (#).", "copy running-config tftp: → svara 192.168.1.10 på frågan om fjärrvärd och tryck Enter på filnamnet.", "Utropstecken och \"bytes copied\" betyder att kopian är klar."],
                check: ck_c6n1, report: None, flag: None },
        ],
        quiz: vec![
            q!("Vad kallas den privata adressen på insidan i NAT-terminologi?", ["Inside local", "Inside global", "Outside global"], "Inside local = den privata adressen (192.168.1.10), inside global = den publika adressen den översätts till (203.0.113.11). Outside local/global beskriver adresser på utsidan."),
            q!("Vad gör overload i ip nat inside source list 1 interface gi0/1 overload?", ["PAT: många inside-adresser delar en publik adress med olika portnummer", "Tillåter fler statiska översättningar", "Stänger av NAT vid hög last"], "Utan overload behövs en publik adress per samtidig inside-värd (dynamisk NAT med pool: ip nat pool)."),
            q!("Vilka är DHCP:s fyra steg?", ["Discover, Offer, Request, Acknowledge", "Request, Reply, Confirm, Close", "Hello, Update, Query, Ack"], "DORA. Discover och Request är broadcast eftersom klienten saknar adress."),
            q!("Vad gör ip helper-address på ett router-interface?", ["Vidarebefordrar DHCP-broadcasts som unicast till en DHCP-server i ett annat nät", "Sätter interfacets gateway", "Aktiverar DNS-servern"], "DHCP-relay behövs när DHCP-servern inte sitter i samma VLAN som klienterna. Routrar skickar annars aldrig broadcast vidare."),
            q!("Vilken syslog-nivå är allvarligast?", ["0 – emergencies", "7 – debugging", "5 – notifications"], "Lägre nummer = allvarligare. Ett meddelande som %LINK-3-UPDOWN har facility LINK och nivå 3 (errors)."),
            q!("Vad betyder stratum i NTP?", ["Avståndet i hopp från en referensklocka – lägre är bättre", "Antalet NTP-klienter", "Tidszonen"], "ntp server <ip> gör enheten till klient. ntp master gör routern till server med sin egen klocka (stratum 8 som standard)."),
            q!("Vilken SNMP-version ger både autentisering och kryptering?", ["SNMPv3 med authPriv", "SNMPv2c", "SNMPv1"], "v1 och v2c skickar communitysträngen i klartext. Polling går till UDP 161, traps och informs från agenten till NMS:en på UDP 162."),
            q!("Vilket DSCP-värde används normalt för rösttrafik?", ["EF (46)", "AF11 (10)", "CS0 (0)"], "QoS: klassa och märk nära källan (DSCP i IP-huvudet, CoS i 802.1Q-taggen), lägg röst i en prioriterad kö (LLQ) och skydda länkarna mot överbelastning."),
            q!("Vad skiljer policing från shaping?", ["Policing kastar eller ommärker trafik över gränsen, shaping buffrar och fördröjer den", "Det är samma sak", "Shaping kastar trafik, policing buffrar den"], "Shaping används typiskt utgående mot en operatör med lägre avtalad hastighet än porten. Policing används ofta inkommande."),
            q!("Vilket protokoll och vilken port använder TFTP?", ["UDP 69", "TCP 21", "TCP 22"], "FTP: TCP 20/21 med inloggning. TFTP: UDP 69 utan inloggning, används för konfigurationer och IOS-avbilder. SSH (och SCP): TCP 22."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 7
    Week {
        week: 207, build: None, kurs: 3,
        title: "Säkerhet", chapter: "C7", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 7 av 8: område 5 i provet, Security Fundamentals (15 %).\n\nPort security med sticky-adresser, en standard-ACL på vty-linjerna, en farlig SNMP-community och DHCP snooping. Frågesporten tar upp hot och sårbarheter, lösenord och MFA, AAA (TACACS+ och RADIUS), ACL-placering, DAI och VPN.",
        learn: vec!["Port security: max, sticky och violation-lägen", "Standard-ACL med wildcard och access-class", "SNMP-communities: RO och RW", "DHCP snooping och trust", "AAA, TACACS+ och RADIUS"],
        chips: vec!["show port-security interface gi0/6", "show access-lists", "show ip dhcp snooping"],
        tasks: vec![
            Task { id: "c7k1", krabba: true, build: false, npc: "Karim", target: "SW1",
                title: "Karims nya dator får inget nät",
                ticket: "Karim: \"Jag fick en ny dator i morse. Den gamla fungerade i samma uttag, men den nya får inget nät alls.\"",
                hints: vec!["show interfaces status: är Gi0/6 err-disabled? show port-security interface gi0/6 visar orsaken och den senast sedda MAC-adressen.", "Porten har max 1 adress och har lärt sig den gamla datorns MAC-adress med sticky. show running-config interface gi0/6 visar den.", "interface gi0/6 → no switchport port-security mac-address sticky 001a.2b3c.4d5e → shutdown → no shutdown. Den nya datorns adress lärs in av sig själv."],
                check: ck_c7k1,
                report: rep!(["Port security (max 1, sticky) hade kvar den gamla datorns MAC-adress, så den nya gav en överträdelse", "BPDU guard stängde porten", "Den nya datorn hade ett trasigt nätverkskort", "Porten låg i fel VLAN"], ["Ta bort den gamla sticky-adressen och öppna porten med shutdown/no shutdown", "Stänga av port security", "Byta nätverkskort", "Byta VLAN"]),
                flag: None },
            Task { id: "c7k2", krabba: true, build: false, npc: "Omar", target: "R1",
                title: "Drift kommer inte in på routern med SSH",
                ticket: "Omar: \"Någon har lagt en access-lista på vty-linjerna på R-Nordvik-1 så att bara driftnätet ska komma åt routern. Bra tanke – men nu kommer inte ens vi in med SSH från teknikerbänken.\"",
                hints: vec!["Prova ssh drift@192.168.1.193 från laptopen. show running-config | section line vty på R-Nordvik-1 visar access-class.", "show access-lists VTY-SKYDD. Vilket nät släpps in? Driftnätet är 192.168.1.192/26, wildcard 0.0.0.63.", "ip access-list standard VTY-SKYDD → no 10 → 10 permit 192.168.1.192 0.0.0.63. Behåll access-class – bara drift ska in."],
                check: ck_c7k2,
                report: rep!(["Standard-ACL:en på vty släppte bara in kontorsnätet 192.168.1.0/26 i stället för driftnätet 192.168.1.192/26", "SSH var avstängt", "RSA-nyckeln saknades", "Fel lösenord"], ["Rätta ACL:en till permit 192.168.1.192 0.0.0.63 och behålla access-class", "Slå på SSH", "Generera en RSA-nyckel", "Byta lösenord"]),
                flag: None },
            Task { id: "c7k3", krabba: true, build: false, npc: "Linnea", target: "R1",
                title: "SNMP-communityn public har skrivrätt",
                ticket: "Linnea: \"Säkerhetsskannern hittade att R-Nordvik-1 svarar på SNMP-communityn public – med skrivrätt! Ta bort den. Övervakningen ska använda communityn nordvik-ro med bara läsrätt.\"",
                hints: vec!["show running-config | include snmp på R-Nordvik-1.", "En community med RW låter vem som helst som gissar strängen ändra konfigurationen. public och private är de första en angripare provar.", "configure terminal → no snmp-server community public → snmp-server community nordvik-ro RO."],
                check: ck_c7k3,
                report: rep!(["SNMP-communityn public hade skrivrätt (RW) – lätt att gissa och farlig", "SNMP var avstängt", "Övervakningen hade fel IP-adress", "Routern saknade NTP"], ["Ta bort public och lägga in nordvik-ro med RO", "Slå på SNMP", "Byta adress på övervakningen", "Lägga in en NTP-server"]),
                flag: None },
            Task { id: "c7n1", krabba: false, build: true, npc: "Omar", target: "SW1",
                title: "DHCP snooping på SW-Nordvik-1",
                ticket: "Omar: \"Förra månaden kopplade någon in en hemmarouter som delade ut fel adresser. Slå på DHCP snooping för VLAN 10, 20 och 30 på SW-Nordvik-1, och lita bara på trunken mot routern (Gi0/1).\"",
                hints: vec!["DHCP snooping släpper bara DHCP-svar (Offer och Ack) på betrodda portar. Alla andra portar får bara skicka klientmeddelanden.", "configure terminal → ip dhcp snooping → ip dhcp snooping vlan 10,20,30.", "interface gi0/1 → ip dhcp snooping trust. Kontrollera med show ip dhcp snooping."],
                check: ck_c7n1, report: None, flag: None },
        ],
        quiz: vec![
            q!("Vad är skillnaden mellan en sårbarhet och en exploit?", ["Sårbarheten är svagheten, exploiten är koden eller metoden som utnyttjar den", "Det är samma sak", "En exploit är ett skydd mot en sårbarhet"], "Hot = någon eller något som kan orsaka skada, sårbarhet = svagheten, exploit = sättet att utnyttja den, mitigering = åtgärden som minskar risken. Ett säkerhetsprogram omfattar också utbildning, fysisk säkerhet och användarmedvetenhet."),
            q!("Varför ska man använda enable secret i stället för enable password?", ["enable secret lagras som en stark hash (typ 5, 8 eller 9)", "enable secret är kortare", "enable password fungerar inte över SSH"], "service password-encryption döljer bara klartextlösenord med den svaga typ 7, som är lätt att knäcka. username drift secret ... ger hashade lokala konton."),
            q!("Vad är multifaktorautentisering (MFA)?", ["Inloggning med minst två olika slags bevis: något du vet, har eller är", "Två lösenord efter varandra", "Inloggning från två datorer"], "Exempel: lösenord plus engångskod i mobilen. Certifikat och biometri är andra faktorer. En lösenordspolicy styr längd, komplexitet och byte."),
            q!("Vad skiljer TACACS+ från RADIUS?", ["TACACS+ använder TCP 49, krypterar hela paketet och separerar AAA-delarna", "TACACS+ är en öppen standard över UDP", "RADIUS krypterar hela paketet"], "RADIUS (UDP 1812/1813) krypterar bara lösenordet och används mycket för nätverksåtkomst (802.1X). TACACS+ passar för administration av enheter, med auktorisering per kommando."),
            q!("Var placeras en standard-ACL normalt?", ["Nära destinationen", "Nära källan", "På alla interface"], "Standard-ACL:er matchar bara källadressen. Utökade (extended) matchar källa, mål, protokoll och port och placeras nära källan."),
            q!("Vad händer med trafik som inte matchar någon rad i en ACL?", ["Den stoppas av den osynliga deny any sist i listan", "Den släpps igenom", "Den loggas och släpps"], "Därför måste en ACL alltid innehålla minst en permit. Ordningen spelar roll – första matchande rad gäller."),
            q!("Vilket port security-läge kastar trafiken, loggar och räknar men låter porten vara uppe?", ["restrict", "protect", "shutdown"], "shutdown (standard) sätter porten i err-disabled, restrict kastar, loggar och räknar, protect kastar tyst."),
            q!("Vad använder Dynamic ARP Inspection (DAI) för att kontrollera ARP-paket?", ["DHCP snoopings bindningstabell", "MAC-tabellen", "Routingtabellen"], "DAI stoppar ARP-spoofing på opålitliga portar. Utan DHCP snooping (eller statiska ARP-ACL:er) har DAI inget att jämföra med."),
            q!("Vad skiljer site-to-site-VPN från remote access-VPN?", ["Site-to-site kopplar ihop hela nät via routrar, remote access kopplar en enskild användare med en klient", "Det är samma sak", "Remote access kräver två routrar"], "Site-to-site IPsec (som Göteborg–Borås i vecka 10) syns inte för användarna. Remote access använder en VPN-klient med IPsec eller TLS."),
            q!("Vad står AAA för?", ["Authentication, Authorization, Accounting", "Access, Audit, Alert", "Administration, Antivirus, ACL"], "Autentisering = vem är du, auktorisering = vad får du göra, accounting = vad gjorde du. aaa new-model slår på AAA i IOS."),
        ],
        challenge: None,
    },
    // ---------------------------------------------------------------- Modul 8
    Week {
        week: 208, build: None, kurs: 3,
        title: "Trådlöst och automation", chapter: "C8", site: "gbg", course: Some("CCNA 200-301"),
        intro: "CCNA, modul 8 av 8: trådlöst ur område 2 och 5, och område 6 i provet, Automation and Programmability (10 %).\n\nGästnätet har blivit öppet, en accesspunkt ligger på en överlappande kanal och Omar vill börja automatisera med controllerns REST-API. Frågesporten går igenom AP-lägen, CAPWAP, WPA2/WPA3, SDN, REST, JSON, Ansible och Terraform.",
        learn: vec!["WLAN-säkerhet: Open, WPA2-PSK och WPA3-SAE", "Kanalerna 1, 6 och 11 i 2,4 GHz", "AP-lägen och CAPWAP", "REST: CRUD, HTTP-verb och JSON", "SDN: kontrollplan, dataplan, north- och southbound"],
        chips: vec!["show wlan summary", "show advanced 802.11b summary", "curl http://192.168.1.196/api/v1/aps"],
        tasks: vec![
            Task { id: "c8k1", krabba: true, build: false, npc: "Nils", target: "WLC",
                title: "Gästnätet är öppet",
                ticket: "Nils: \"Kunderna på lagret säger att gästnätet Nordvik-Gast inte frågar efter något lösenord längre. Är det meningen?\"",
                hints: vec!["Logga in på controllern (ssh från laptopen eller konsolen) och kör show wlan summary. Vilken säkerhet har WLAN 2?", "Open betyder ingen kryptering alls – vem som helst kan ansluta och avlyssna. WPA2-PSK använder AES (CCMP) med en delad nyckel, WPA3-SAE skyddar dessutom mot gissning av nyckeln.", "config wlan disable 2 → config wlan security wpa2-psk 2 → config wlan enable 2 → save config."],
                check: ck_c8k1,
                report: rep!(["WLAN 2 (Nordvik-Gast) var satt till Open utan kryptering", "Accesspunkterna var urkopplade", "Gästernas DHCP-pool var full", "Gäst-VLAN:et saknades"], ["Sätta WPA2-PSK (eller WPA3-SAE) på WLAN 2 och spara", "Koppla in accesspunkterna", "Göra poolen större", "Skapa VLAN:et"]),
                flag: None },
            Task { id: "c8k2", krabba: true, build: false, npc: "Nils", target: "WLC",
                title: "Wi-Fi:t mitt i lagret hackar",
                ticket: "Nils: \"Skannrarna tappar Wi-Fi hela tiden mitt i lagret, mellan hylla 2 och 3.\"",
                hints: vec!["show advanced 802.11b summary eller show ap summary på controllern visar kanalerna.", "I 2,4 GHz överlappar kanalerna varandra. Bara 1, 6 och 11 är helt fria från varandra – kanal 3 stör både 1 och 6.", "config 802.11b channel ap AP-Lager-2 6, så att de tre accesspunkterna har 1, 6 och 11."],
                check: ck_c8k2,
                report: rep!(["AP-Lager-2 låg på kanal 3, som överlappar både kanal 1 och 6", "Accesspunkterna hade för låg sändeffekt", "Skannrarna hade fel lösenord", "Controllern var överbelastad"], ["Sätta AP-Lager-2 på kanal 6 så att kanalerna blir 1, 6 och 11", "Höja sändeffekten till max", "Byta lösenord", "Starta om controllern"]),
                flag: None },
            Task { id: "c8n1", krabba: false, build: false, npc: "Omar", target: "Tekniker",
                title: "Hämta AP-listan via REST-API:t",
                ticket: "Omar: \"Vi ska automatisera övervakningen av accesspunkterna. Controllern har ett REST-API. Hämta listan med curl från laptopen och kolla att JSON-svaret ser vettigt ut.\"",
                hints: vec!["REST-API:er använder HTTP-verben: GET läser, POST skapar, PUT/PATCH ändrar och DELETE tar bort (CRUD).", "Från laptopen: curl http://192.168.1.196/api/v1/aps. Svaret är JSON: objekt i { }, listor i [ ], \"nyckel\": värde.", "Läs kanal och status för varje accesspunkt i svaret och skriv felrapporten (F)."],
                check: ck_c8n1,
                report: rep!(["GET /api/v1/aps gav en JSON-lista med alla tre accesspunkterna: adress, kanal och status", "API:t krävde POST för att läsa data", "Svaret var XML med <ap>-taggar", "API:t svarade 404 Not Found"], ["Läsa data med GET och tolka JSON-svaret", "Skicka DELETE för att uppdatera listan", "Skriva om svaret till CSV för hand", "Starta om controllern"]),
                flag: Some("apiAps") },
        ],
        quiz: vec![
            q!("Vilka kanaler i 2,4 GHz överlappar inte varandra?", ["1, 6 och 11", "1, 2 och 3", "1, 5 och 9"], "5 GHz har många fler icke-överlappande kanaler. RF-begrepp: SSID är nätets namn, BSSID AP:ns MAC-adress per radio."),
            q!("Vilket AP-läge används för vanliga klienter med en central controller?", ["Local", "Monitor", "Sniffer"], "FlexConnect låter ett AP på ett avlägset kontor switcha trafiken lokalt om länken till controllern går ner. Monitor och sniffer betjänar inga klienter; bridge används för trådlösa länkar mellan byggnader."),
            q!("Vilket protokoll använder lightweight-AP:er mot controllern?", ["CAPWAP (UDP 5246 och 5247)", "SNMP", "LLDP"], "Kontrolltrafiken (UDP 5246) är krypterad, datatrafiken går på 5247. Autonoma AP:er konfigureras var för sig utan controller; molnstyrda (som Meraki) hanteras från en webbportal."),
            q!("Hur är switchporten mot en WLC normalt konfigurerad?", ["Som trunk, ofta i en EtherChannel (LAG), eftersom WLAN:en mappas till olika VLAN", "Som accessport i VLAN 1", "Med bara PortFast och BPDU guard"], "Ett AP i local-läge sitter på en accessport eftersom allt tunnlas i CAPWAP. Ett FlexConnect-AP behöver en trunk om flera WLAN switchas lokalt."),
            q!("Vad ger WPA3-Personal?", ["SAE, som skyddar mot offline-gissning av lösenordet", "TKIP", "WEP med 128-bitarsnyckel"], "WPA: TKIP (föråldrat). WPA2: AES-CCMP med PSK eller 802.1X (Enterprise). WPA3: SAE i Personal, 192-bitarsläge i Enterprise och skyddade management-ramar (PMF)."),
            q!("Vad skiljer controllerbaserade nät (SDN) från traditionella?", ["Kontrollplanet samlas i en controller som styr enheterna via API:er", "Varje enhet konfigureras för hand via CLI", "Det finns inget dataplan"], "Traditionellt har varje enhet eget kontroll- och dataplan. Med SDN fattar controllern besluten medan vidarebefordringen (dataplanet) ligger kvar i enheterna. Automation ger färre manuella fel och snabbare, enhetliga ändringar."),
            q!("Vad är ett northbound API?", ["Gränssnittet mellan controllern och applikationerna ovanför, ofta REST", "Gränssnittet mellan controllern och nätverksenheterna", "En trunk mot core-switchen"], "Southbound: controller → enheter (NETCONF, RESTCONF, OpenFlow, CLI/SNMP). Underlay = det fysiska nätet, overlay = virtuella tunnlar ovanpå (t.ex. VXLAN), fabric = helheten."),
            q!("Vilket HTTP-verb skapar en ny resurs i ett REST-API?", ["POST", "GET", "DELETE"], "CRUD: Create = POST, Read = GET, Update = PUT/PATCH, Delete = DELETE. Svarskoder: 200 OK, 201 Created, 401 Unauthorized, 403 Forbidden, 404 Not Found. Autentisering: basic, API-nyckel/token eller OAuth."),
            q!("Vilket exempel är giltig JSON?", ["{\"ap\": \"AP-Lager-1\", \"kanal\": 6, \"aktiv\": true}", "{ap = AP-Lager-1; kanal = 6}", "<ap>AP-Lager-1</ap>"], "JSON: objekt i { }, listor i [ ], nycklar som strängar i citattecken och värden som sträng, tal, true/false, null, objekt eller lista. Det sista exemplet är XML."),
            q!("Vad kännetecknar Ansible?", ["Agentlöst, playbooks i YAML, skickar konfiguration över SSH", "Kräver en agent på varje enhet och skrivs i Ruby", "Ett övervakningsverktyg för SNMP"], "Terraform är deklarativ infrastruktur som kod (HCL) som jämför önskat och faktiskt läge. Puppet och Chef använder agenter. AI i drift: prediktiv AI och maskininlärning förutser fel ur mätdata, generativ AI skapar t.ex. konfigurationer som måste granskas."),
        ],
        challenge: None,
    },
    ]
}
