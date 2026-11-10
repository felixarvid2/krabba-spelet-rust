//! Wlc-sessionen. Port av js/cli/wlc.js (och WLC-tilläggen i js/cli/more7.js).
use super::{Cli, CmdResult};
use crate::net::model::{Wlc, WlcSaved};
use crate::net::sim::{self, PingOpts, State};
use crate::net::util as U;

const HELP: &str = "config         Configure switch options and settings.\nsave           Save switch configurations.\nshow           Display switch options and settings.\nlogout         Exit this session.";
const HELP_EXTRA: &str = "\n\nFler show-kommandon: show client summary, show ap config general <ap>, show time, show network summary\nping <ip>";
const NO_CMD: &str = "\nIncorrect usage. Use the '?' or <TAB> key to list commands.";
const NETWORK: &str = "\nRF-Network Name............................. Nordvik\nWeb Mode.................................... Disable\nSecure Web Mode............................. Enable\nSecure Shell (ssh).......................... Enable\nTelnet...................................... Disable\nEthernet Multicast Mode..................... Disable\nAP Join Priority............................ Disable";
const DOW: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Väntande fråga (JS: this.pending).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    User,
    Password,
    Unsaved,
    Confirm,
}

fn ask_text(a: Ask) -> &'static str {
    match a {
        Ask::User => "User: ",
        Ask::Password => "Password:",
        Ask::Unsaved => "The system has unsaved changes.\nWould you like to save them now? (y/N) ",
        Ask::Confirm => "\n\nAre you sure you want to save? (y/n) ",
    }
}

pub struct WlcSession {
    pub dev: String,
    pub closed: bool,
    pending: Option<Ask>,
}

impl WlcSession {
    pub fn new(st: &State, dev: &str) -> WlcSession {
        let _ = st;
        WlcSession { dev: dev.to_string(), closed: false, pending: Some(Ask::User) }
    }
    fn w<'a>(&self, st: &'a State) -> &'a Wlc {
        st.dev(&self.dev).wlc.as_ref().expect("WLC-enhet")
    }
    fn w_mut<'a>(&self, st: &'a mut State) -> &'a mut Wlc {
        st.dev_mut(&self.dev).wlc.as_mut().expect("WLC-enhet")
    }
    fn close(&mut self) -> CmdResult {
        self.closed = true;
        CmdResult { close: true, ..Default::default() }
    }
    fn save_cfg(&self, st: &mut State) {
        let w = self.w_mut(st);
        w.saved = Some(WlcSaved { wlans: w.wlans.clone(), ap_channels: w.ap_channels.clone() });
    }
    fn answer(&mut self, st: &mut State, p: Ask, x: &str) -> CmdResult {
        let yes = x.to_lowercase().starts_with('y');
        match p {
            Ask::User => {
                self.pending = Some(Ask::Password);
                CmdResult::out("")
            }
            Ask::Password => CmdResult::out(""),
            Ask::Unsaved => {
                if yes {
                    self.save_cfg(st);
                }
                self.close()
            }
            Ask::Confirm => {
                if !yes {
                    return CmdResult::out("");
                }
                self.save_cfg(st);
                CmdResult::out("\n\nConfiguration Saved!")
            }
        }
    }

    fn show(&self, st: &mut State, a: &[&str], exact: bool) -> String {
        let t = a[1..].join(" ").to_lowercase();
        // Tilläggen från more7.js (endast när kommandot skrivs "show" exakt)
        if exact {
            if after_kw(&t, &["client"]).is_some_and(|r| r.starts_with('s')) {
                return self.client_summary(st);
            }
            if after_kw(&t, &["ap"]).and_then(|r| r.strip_prefix("config")).and_then(ws).is_some_and(|r| r.starts_with("general")) {
                return self.ap_config_general(st, a.get(4).copied());
            }
            if t.starts_with("time") {
                return time_text(st);
            }
            if after_kw(&t, &["network", "net"]).is_some_and(|r| r.starts_with('s')) {
                return NETWORK.into();
            }
        }
        if after_kw(&t, &["wlan", "wl"]).is_some_and(|r| r.starts_with('s')) {
            return wlan_summary(self.w(st));
        }
        if after_kw(&t, &["wlan", "wl"]).is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
            return wlan_detail(self.w(st), parse_int(a.get(2).copied()));
        }
        if after_kw(&t, &["interface", "int"]).is_some_and(|r| r.starts_with('s')) {
            return if_summary(self.w(st));
        }
        if after_kw(&t, &["ap"]).is_some_and(|r| r.starts_with('s')) {
            return self.ap_summary(st);
        }
        let adv = after_kw(&t, &["advanced", "adv"])
            .and_then(|r| r.strip_prefix("802.11b"))
            .and_then(ws)
            .is_some_and(|r| r.starts_with('s'));
        if adv || after_kw(&t, &["ap"]).is_some_and(|r| r.starts_with("ch")) {
            return self.channels(st);
        }
        if t.starts_with("sys") {
            return sysinfo(self.w(st));
        }
        NO_CMD.into()
    }

    fn config(&self, st: &mut State, a: &[&str]) -> String {
        let sub = a.get(1).copied().unwrap_or("").to_lowercase();
        if sub == "wlan" {
            let op = a.get(2).copied().unwrap_or("").to_lowercase();
            let id = parse_int(a.get(3).copied());
            let key = id.and_then(|i| u32::try_from(i).ok());
            let exists = key.is_some_and(|k| self.w(st).wlans.contains_key(&k));
            if let (true, Some(k)) = (exists, key) {
                if op == "enable" || op == "disable" {
                    self.w_mut(st).wlans.get_mut(&k).expect("WLAN").enabled = op == "enable";
                    sim::touch(st);
                    return String::new();
                }
                if op == "interface" {
                    let ifn = a.get(4).copied().unwrap_or("");
                    let w = self.w(st);
                    if !w.interfaces.contains_key(ifn) {
                        return "\nInterface does not exist.".into();
                    }
                    if w.wlans[&k].enabled {
                        return format!("\nRequest failed: WLAN must be disabled before changing its interface.\n(Use \"config wlan disable {}\" first.)", k);
                    }
                    self.w_mut(st).wlans.get_mut(&k).expect("WLAN").iface = ifn.to_string();
                    sim::touch(st);
                    return String::new();
                }
            }
            if !exists && id.is_some_and(|i| i != 0) {
                return "\nWLAN identifier is invalid.".into();
            }
            return "\nIncorrect usage. Use: config wlan {enable|disable} <id>  |  config wlan interface <id> <interface-name>".into();
        }
        if sub == "802.11b" && a.get(2).copied().unwrap_or("").to_lowercase() == "channel" && a.get(3).copied().unwrap_or("").to_lowercase() == "ap" {
            let ap = a.get(4).copied().unwrap_or("undefined");
            let ch = parse_int(a.get(5).copied());
            if !self.w(st).ap_channels.contains_key(ap) {
                return format!("\nAP {} is not associated.", ap);
            }
            let c = match ch {
                Some(c) if (1..=13).contains(&c) => c,
                _ => return "\nInvalid channel. Channels 1-13 are allowed in regulatory domain -E.".into(),
            };
            self.w_mut(st).ap_channels.insert(ap.to_string(), c as u32);
            sim::touch(st);
            return String::new();
        }
        NO_CMD.into()
    }

    fn ap_summary(&self, st: &mut State) -> String {
        let d = sim::get(st);
        let mut names: Vec<&String> = d.ap_joined.iter().filter(|(_, v)| **v).map(|(k, _)| k).collect();
        names.sort();
        let mut o: Vec<String> = vec![
            String::new(),
            format!("Number of APs.................................... {}", names.len()),
            String::new(),
            "Global AP User Name.............................. admin".into(),
            String::new(),
            "AP Name             Slots  AP Model              Ethernet MAC       Location          Country  IP Address       Clients".into(),
            "------------------  -----  --------------------  -----------------  ----------------  -------  ---------------  -------".into(),
        ];
        for n in names {
            let dev = st.dev(n);
            let clients = d.wifi.values().filter(|x| x.ap == *n).count();
            let ip = dev.nic.as_ref().and_then(|x| x.static_.as_ref()).map(|s| s.ip.as_str()).unwrap_or("");
            o.push(format!(
                "{}{}{}{}{}{}{}{}",
                U::pad(n, 20),
                U::pad("2", 7),
                U::pad("AIR-CAP3702I-E-K9", 22),
                U::pad(U::mac_colon(&dev.mac()), 19),
                U::pad("Boras lager", 18),
                U::pad("SE", 9),
                U::pad(ip, 17),
                clients
            ));
        }
        o.join("\n")
    }

    fn channels(&self, st: &mut State) -> String {
        let d = sim::get(st);
        let w = self.w(st);
        let mut o: Vec<String> = vec![
            "Member RRM Information".into(),
            "AP Name            MAC Address        Slot   Admin   Oper    Channel   TxPower".into(),
            "------------------ ------------------ ----- -------- ------- --------- -------".into(),
        ];
        let mut keys: Vec<&String> = w.ap_channels.keys().collect();
        keys.sort();
        for n in keys {
            if d.ap_joined.get(n) != Some(&true) {
                continue;
            }
            let dev = st.dev(n);
            o.push(format!(
                "{}{}{}{}{}{}*1/8 (20 dBm)",
                U::pad(n, 19),
                U::pad(U::mac_colon(&dev.mac()), 19),
                U::pad("0", 6),
                U::pad("ENABLED", 9),
                U::pad("UP", 8),
                U::pad(w.ap_channels[n], 10)
            ));
        }
        o.join("\n")
    }

    fn client_summary(&self, st: &mut State) -> String {
        let d = sim::get(st);
        let w = self.w(st);
        let aps: Vec<&String> = d.ap_joined.keys().collect();
        let mut rows: Vec<String> = vec![
            String::new(),
            "Number of Clients................................ ".into(),
            String::new(),
            "MAC Address       AP Name           Slot Status        WLAN  Auth Protocol         Port Wired Tunnel  Role".into(),
            "----------------- ----------------- ---- ------------- ----- ---- ---------------- ---- ----- ------- --------".into(),
        ];
        let mut n = 0;
        for (id, h) in st.devices.iter() {
            let Some(nic) = &h.nic else { continue };
            if !h.wireless {
                continue;
            }
            let conf = sim::host_ip_conf(h).is_some();
            let no_ssid = h.ssid.as_deref().map_or(true, |s| s.is_empty());
            let wl = w
                .wlans
                .iter()
                .find(|(_, x)| h.ssid.as_deref() == Some(x.ssid.as_str()) || (no_ssid && x.enabled))
                .map(|(k, _)| k.to_string())
                .unwrap_or_else(|| "1".into());
            let ap = if aps.is_empty() { "-" } else { aps[(U::hash(id) as usize) % aps.len()].as_str() };
            n += 1;
            rows.push(format!(
                "{}{}{}{}{}{}{}{}{}{}Local",
                U::pad(U::mac_colon(&nic.mac), 18),
                U::pad(ap, 18),
                U::pad("1", 5),
                U::pad(if conf { "Associated" } else { "Probing" }, 14),
                U::pad(wl, 6),
                U::pad("Yes", 5),
                U::pad("802.11n(2.4 GHz)", 17),
                U::pad("1", 5),
                U::pad("No", 6),
                U::pad("No", 8),
            ));
        }
        rows[1].push_str(&n.to_string());
        rows.join("\n")
    }

    fn ap_config_general(&self, st: &mut State, apn: Option<&str>) -> String {
        let d = sim::get(st);
        let w = self.w(st);
        let Some(apn) = apn.filter(|a| st.devices.contains_key(*a)) else {
            return "\nCisco AP name is invalid.".into();
        };
        let joined = d.ap_joined.get(apn) == Some(&true);
        let ch = match w.ap_channels.get(apn) {
            Some(c) if *c != 0 => c.to_string(),
            _ => "auto".into(),
        };
        format!(
            "\nCisco AP Name.................................... {}\nCountry code..................................... SE  - Sweden\nAP Mode ......................................... Local\nAP Role ......................................... {}\nIP Address Configuration......................... DHCP\nSwitch Port Number .............................. 1\nAP Model......................................... AIR-CAP3702I-E-K9\nPower Type/Mode.................................. PoE/Full Power\nChannel.......................................... {}",
            apn,
            if joined { "Joined" } else { "Not joined" },
            ch
        )
    }
}

impl Cli for WlcSession {
    fn prompt_text(&self, st: &State) -> String {
        match self.pending {
            Some(p) => ask_text(p).into(),
            None => format!("({}) >", self.w(st).sys_name),
        }
    }

    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        if let Some(p) = self.pending.take() {
            return self.answer(st, p, line.trim());
        }
        let a: Vec<&str> = line.split_whitespace().collect();
        if a.is_empty() {
            return CmdResult::out("");
        }
        let c0 = a[0].to_lowercase();
        let a1 = a.get(1).copied().unwrap_or("");

        // ping (more7.js)
        if c0 == "ping" {
            if let Some(&dst) = a.get(1) {
                let p = sim::ping(st, &self.dev, dst, PingOpts::default());
                return CmdResult {
                    out: format!("\nSend count=3, Receive count={} from {}\n", if p.ok { 3 } else { 0 }, dst),
                    delay: if p.ok { 600 } else { 2500 },
                    ..Default::default()
                };
            }
        }
        if m(&c0, "logout") || m(&c0, "exit") {
            if changed(self.w(st)) {
                self.pending = Some(Ask::Unsaved);
                return CmdResult::out("");
            }
            return self.close();
        }
        if m(&c0, "save") && m(a1, "config") {
            self.pending = Some(Ask::Confirm);
            return CmdResult::out("");
        }
        if m(&c0, "show") {
            return CmdResult::out(self.show(st, &a, c0 == "show"));
        }
        if m(&c0, "config") {
            return CmdResult::out(self.config(st, &a));
        }
        if c0 == "?" || c0 == "help" {
            return CmdResult::out(self.help_text());
        }
        CmdResult::out(NO_CMD)
    }

    fn help(&mut self, st: &mut State, line: &str) -> String {
        let _ = (st, line);
        self.help_text()
    }

    fn secret_input(&self) -> bool {
        self.pending == Some(Ask::Password)
    }

    fn closed(&self) -> bool {
        self.closed
    }

    fn dev_id(&self) -> &str {
        &self.dev
    }
}

impl WlcSession {
    fn help_text(&self) -> String {
        format!("{}{}", HELP, HELP_EXTRA)
    }
}

/// JS m(word, full): ordet är ett prefix av kommandot (gemener), ordet får inte vara tomt.
fn m(word: &str, full: &str) -> bool {
    !word.is_empty() && full.starts_with(&word.to_lowercase())
}

/// Blanksteg efter ordet, och resten efter blanksteget.
fn ws(t: &str) -> Option<&str> {
    if t.starts_with(char::is_whitespace) { Some(t.trim_start()) } else { None }
}

/// Motsvarar regex `^(alt)\s+`: ger resten efter prefixet och blanksteget.
fn after_kw<'a>(t: &'a str, alts: &[&str]) -> Option<&'a str> {
    alts.iter().find_map(|a| t.strip_prefix(*a).and_then(ws))
}

/// JS parseInt(s, 10): inledande heltal, annars None.
fn parse_int(s: Option<&str>) -> Option<i64> {
    let s = s?.trim_start();
    let mut end = 0;
    for (i, c) in s.char_indices() {
        let ok = c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+'));
        if !ok {
            break;
        }
        end = i + c.len_utf8();
    }
    s[..end].parse().ok()
}

fn changed(w: &Wlc) -> bool {
    match &w.saved {
        None => true,
        Some(s) => s.wlans != w.wlans || !s.ap_channels.iter().eq(w.ap_channels.iter()),
    }
}

fn wlan_summary(w: &Wlc) -> String {
    let mut o: Vec<String> = vec![
        String::new(),
        format!("Number of WLANs.................................. {}", w.wlans.len()),
        String::new(),
        "WLAN ID  WLAN Profile Name / SSID               Status    Interface Name        PMIPv6 Mobility".into(),
        "-------  -------------------------------------  --------  --------------------  ---------------".into(),
    ];
    for (id, wl) in &w.wlans {
        o.push(format!(
            "{}{}{}{}none",
            U::pad(id, 9),
            U::pad(format!("{} / {}", wl.ssid, wl.ssid), 39),
            U::pad(if wl.enabled { "Enabled" } else { "Disabled" }, 10),
            U::pad(&wl.iface, 22)
        ));
    }
    o.join("\n")
}

fn wlan_detail(w: &Wlc, id: Option<i64>) -> String {
    let Some(key) = id.and_then(|i| u32::try_from(i).ok()).filter(|k| w.wlans.contains_key(k)) else {
        return "\nWLAN identifier is invalid.".into();
    };
    let wl = &w.wlans[&key];
    let vlan = w.interfaces.get(&wl.iface).map(|i| i.vlan.to_string()).unwrap_or_else(|| "?".into());
    [
        String::new(),
        format!("WLAN Identifier.................................. {}", key),
        format!("Profile Name..................................... {}", wl.ssid),
        format!("Network Name (SSID).............................. {}", wl.ssid),
        format!("Status........................................... {}", if wl.enabled { "Enabled" } else { "Disabled" }),
        format!("Interface........................................ {}", wl.iface),
        format!("VLAN............................................. {}", vlan),
        "Security".into(),
        "   802.11 Authentication:........................ Open System".into(),
        "   Static WEP Keys............................... Disabled".into(),
        "   Wi-Fi Protected Access (WPA/WPA2)............. Enabled".into(),
    ]
    .join("\n")
}

fn if_summary(w: &Wlc) -> String {
    let mut o: Vec<String> = vec![
        String::new(),
        format!("Number of Interfaces.......................... {}", w.interfaces.len()),
        String::new(),
        "Interface Name                   Port Vlan Id  IP Address      Type    Ap Mgr Guest".into(),
        "-------------------------------- ---- -------- --------------- ------- ------ -----".into(),
    ];
    let mut names: Vec<&String> = w.interfaces.keys().collect();
    names.sort();
    for n in names {
        let i = &w.interfaces[n];
        let mgmt = n.as_str() == "management";
        o.push(format!(
            "{}{}{}{}{}{}No",
            U::pad(n, 33),
            U::pad("1", 5),
            U::pad(i.vlan, 9),
            U::pad(&i.ip, 16),
            U::pad(if mgmt { "Static" } else { "Dynamic" }, 8),
            U::pad(if mgmt { "Yes" } else { "No" }, 7)
        ));
    }
    o.join("\n")
}

fn sysinfo(w: &Wlc) -> String {
    format!(
        "Manufacturer's Name.............................. Cisco Systems Inc.\nProduct Name..................................... Cisco Controller\nProduct Version.................................. 7.0.252.0\nSystem Name...................................... {}\nIP Address....................................... 192.168.1.196\nNumber of WLANs.................................. {}",
        w.sys_name,
        w.wlans.len()
    )
}

/// Klockan i WLC:n: golden-tiden 2026-09-29 08:00 UTC plus simulerad tid (JS Date.toUTCString utan "GMT").
fn time_text(st: &State) -> String {
    let p = U::date_parts(U::date_utc(2026, 8, 29, 8, 0, 0) + st.time * 1000.0);
    let stamp = format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} ",
        DOW[p.dow as usize], p.day, MON[p.mon0 as usize], p.year, p.h, p.m, p.s
    );
    format!(
        "\nTime............................................. {}\nTimezone delta................................... 0:0\nNTP Servers\n    NTP Polling Interval.........................  3600\n     Index     NTP Key Index      NTP Server       Status\n    -------  ---------------------------------------------------\n       1              0            192.168.1.16     In Sync",
        stamp
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::model;

    fn setup() -> (State, WlcSession) {
        let st = State::new(model::build_golden(8));
        let s = WlcSession::new(&st, "WLC");
        (st, s)
    }

    fn login(st: &mut State, s: &mut WlcSession) {
        s.handle(st, "admin");
        s.handle(st, "geheim");
    }

    #[test]
    fn inloggning_och_prompt() {
        let (mut st, mut s) = setup();
        assert_eq!(s.prompt_text(&st), "User: ");
        s.handle(&mut st, "admin");
        assert_eq!(s.prompt_text(&st), "Password:");
        assert!(s.secret_input());
        s.handle(&mut st, "geheim");
        assert_eq!(s.prompt_text(&st), "(WLC-Nordvik) >");
        assert!(!s.secret_input());
    }

    #[test]
    fn wlan_gar_inte_att_byta_gränssnitt_när_den_är_på() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        let out = s.handle(&mut st, "config wlan interface 2 lager").out;
        assert!(out.contains("must be disabled"), "{}", out);
        assert_eq!(st.dev("WLC").wlc.as_ref().unwrap().wlans[&2].iface, "gast");
    }

    #[test]
    fn config_wlan_interface_andrar_konfigurationen() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        s.handle(&mut st, "config wlan disable 2");
        assert!(!st.dev("WLC").wlc.as_ref().unwrap().wlans[&2].enabled);
        let out = s.handle(&mut st, "config wlan interface 2 lager");
        assert_eq!(out.out, "");
        assert_eq!(st.dev("WLC").wlc.as_ref().unwrap().wlans[&2].iface, "lager");
        assert!(s.handle(&mut st, "config wlan interface 2 finns-inte").out.contains("Interface does not exist"));
        assert!(s.handle(&mut st, "config wlan enable 9").out.contains("WLAN identifier is invalid"));
    }

    #[test]
    fn kanal_pa_ap() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        s.handle(&mut st, "config 802.11b channel ap AP-Lager-2 11");
        assert_eq!(st.dev("WLC").wlc.as_ref().unwrap().ap_channels["AP-Lager-2"], 11);
        assert!(s.handle(&mut st, "config 802.11b channel ap AP-Lager-2 14").out.contains("Invalid channel"));
        assert!(s.handle(&mut st, "config 802.11b channel ap OKAND 3").out.contains("is not associated"));
    }

    #[test]
    fn wlan_och_granssnitt_visas() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        let w = s.handle(&mut st, "show wlan summary").out;
        assert!(w.contains("Nordvik-Gast") || w.contains("Nordvik-Lager"));
        let i = s.handle(&mut st, "show interface summary").out;
        assert!(i.contains("management"));
        let d = s.handle(&mut st, "show wlan 2").out;
        assert!(d.contains("WLAN Identifier.................................. 2"));
    }

    #[test]
    fn save_och_logout_med_fraga() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        s.handle(&mut st, "config wlan disable 1");
        let r = s.handle(&mut st, "logout");
        assert!(!r.close);
        assert!(s.prompt_text(&st).starts_with("The system has unsaved changes."));
        let r = s.handle(&mut st, "y");
        assert!(r.close);
        assert!(s.closed());
        assert!(!st.dev("WLC").wlc.as_ref().unwrap().wlans[&1].enabled);
        // "y" på frågan sparar den aktuella konfigurationen (wlan 1 avstängd)
        assert_eq!(st.dev("WLC").wlc.as_ref().unwrap().saved.as_ref().unwrap().wlans[&1].enabled, false);
    }

    #[test]
    fn save_config_bekraftas() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        s.handle(&mut st, "config wlan disable 1");
        s.handle(&mut st, "save config");
        assert!(s.prompt_text(&st).contains("Are you sure you want to save? (y/n)"));
        let r = s.handle(&mut st, "y");
        assert_eq!(r.out, "\n\nConfiguration Saved!");
        assert_eq!(st.dev("WLC").wlc.as_ref().unwrap().saved.as_ref().unwrap().wlans[&1].enabled, false);
    }

    #[test]
    fn okant_kommando_och_hjalp() {
        let (mut st, mut s) = setup();
        login(&mut st, &mut s);
        assert!(s.handle(&mut st, "foo").out.starts_with("\nIncorrect usage"));
        assert!(s.handle(&mut st, "help").out.contains("logout         Exit this session."));
    }
}
