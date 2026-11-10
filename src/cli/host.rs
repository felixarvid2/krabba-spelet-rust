//! Terminaler på datorerna: Windows (svensk kommandotolk) och teknikerns Linux-laptop.
//! Port av js/cli/host.js och js/cli/host_more.js.
//!
//! Filer: host.rs (sessionen, gemensamma hjälpare, curl, pipe, komplettering),
//! host_win.rs (Windows, grundkommandon), host_lin.rs (Linux, grundkommandon, ssh/telnet/screen),
//! host_more.rs (tilläggen från host_more.js för båda).
//!
//! Publika fält som spelet sätter/läser (se HostSession):
//! - `console_target`: enheten som konsolkabeln sitter i (NV.game.consoleTarget()). Sätts av spelet
//!   före varje kommando; `screen /dev/ttyUSB0` och `ls /dev/ttyUSB*` använder den.
//! - `events`: händelser till spelet (töms av spelet): `cmd:<rad>` för varje inmatat kommando
//!   (NV.onHostCommand, räknas en gång per rad) och `telnet:<enhet>` (NV.onTelnet).
#![allow(unused_variables, dead_code)]
use super::{Chunk, Cli, CmdResult, PushReq};
use crate::net::model::*;
use crate::net::sim::{self as S, IpConf, State};
use crate::net::util as U;
use indexmap::IndexMap;

pub(super) const FORTUNES: &[&str] = &[
    "Det är alltid DNS. Utom när det är kabeln.",
    "Glöm inte write memory – strömmen går alltid när du minst anar det.",
    "Pinga gatewayen först. Sedan allt annat.",
    "En switchport i fel VLAN ser helt frisk ut. Det är det som är problemet.",
    "Den som dokumenterar slipper felsöka samma sak två gånger.",
    "show logging vet mer än du tror.",
    "Om det fungerade i går: vad ändrades i natt?",
    "Ett loopback-interface går aldrig ner. Det går inte att säga om resten av nätet.",
    "Den som kör debug all på en produktionsrouter får lära sig vad CPU betyder.",
    "Stavar du fel kommer terminalen att föreslå rätt. Livet är inte lika snällt.",
    "nmap utan tillstånd är ett samtal med juristen. nmap med tillstånd är felsökning.",
    "Kabeltest först, teori sedan.",
    "En ACL som släpper allt är ingen ACL. En ACL som stoppar allt är ett ärende.",
    "Katten på kontoret är inte ett säkerhetshot. Troligen.",
    "Skriv felrapporten medan du minns vad du gjorde.",
    "Native VLAN 1 är som att lämna nyckeln under dörrmattan.",
    "Det finns två sorters tekniker: de som tar backup och de som kommer att göra det.",
];

/// Väntande fråga (lösenord, värdnyckel, användarnamn ...).
pub(super) struct Pending {
    pub prompt: String,
    pub secret: bool,
    pub kind: PendKind,
}

pub(super) enum PendKind {
    HostKey { host: String, ip: String, dev: String, user: String },
    SshPass { host: String, dev: String, user: String },
    LbPass { dev: String },
    TelUser { dev: String },
    TelPass { dev: String, user: String },
    ScpPass { host: String, dev: String, user: String, path: String, dst: String },
}

/// En session på en värd (Windows-dator, Linux-laptop, server).
pub struct HostSession {
    pub dev: String,
    pub closed: bool,
    /// Enheten som konsolkabeln sitter i (spelets consoleTarget). Spelet sätter detta före varje kommando.
    pub console_target: Option<String>,
    /// Händelser till spelet: "cmd:<rad>" per kommando, "telnet:<enhet>". Spelet tömmer listan.
    pub events: Vec<String>,
    pub(super) pending: Option<Pending>,
    pub(super) user: String,
    pub(super) linux: bool,
    pub(super) hist: Vec<String>,
    pub(super) whist: Vec<String>,
    /// Kända SSH-värdar (nyckel = det som skrevs, oftast ip), i insättningsordning
    pub(super) known: Vec<String>,
    pub(super) files: IndexMap<String, String>,
    pub(super) dns_cache: IndexMap<String, (String, f64)>,
    pub(super) tries: u32,
    pub(super) pw_shown: bool,
}

impl HostSession {
    /// Ny session på värden (os avgör Windows eller Linux).
    pub fn new(st: &State, dev: &str) -> HostSession {
        let d = st.dev(dev);
        let linux = d.os == Os::Linux || dev == "Tekniker";
        let user = {
            let u = d.user.clone().unwrap_or_else(|| "user".into()).to_lowercase();
            u.replacen('ä', "a", 1).replacen('å', "a", 1).replacen('ö', "o", 1)
        };
        HostSession {
            dev: dev.to_string(),
            closed: false,
            console_target: None,
            events: vec![],
            pending: None,
            user,
            linux,
            hist: vec![],
            whist: vec![],
            known: vec![],
            files: IndexMap::new(),
            dns_cache: IndexMap::new(),
            tries: 0,
            pw_shown: false,
        }
    }
    /// Text som visas när terminalen öppnas (t.ex. "Microsoft Windows [Version ...]")
    pub fn banner(&self, st: &State) -> String {
        if self.linux {
            "Ubuntu 24.04 LTS  —  teknikerns laptop\nTips: sudo screen /dev/ttyUSB0 9600   öppnar konsolen på enheten där konsolkabeln sitter.\n      Skriv help för fler kommandon.\n".into()
        } else {
            "Microsoft Windows [Version 10.0.19045.4894]\n(c) Microsoft Corporation. Med ensamrätt.\n".into()
        }
    }
    pub fn is_linux(&self) -> bool {
        self.linux
    }
    pub(super) fn me<'a>(&self, st: &'a State) -> &'a Device {
        st.dev(&self.dev)
    }
    fn win_prompt(&self) -> String {
        format!("C:\\Users\\{}>", self.user)
    }
}

// ------------------------------------------------------------------ Små hjälpare

pub(super) fn r(s: impl Into<String>) -> CmdResult {
    CmdResult::out(s)
}
pub(super) fn rd(s: impl Into<String>, delay: u32) -> CmdResult {
    CmdResult::delayed(s, delay)
}
pub(super) fn ck(text: impl Into<String>, delay: u32) -> Chunk {
    Chunk { text: text.into(), delay }
}
pub(super) fn rs(stream: Vec<Chunk>) -> CmdResult {
    CmdResult::stream(stream)
}
pub(super) fn conf(h: &Device) -> Option<IpConf> {
    S::host_ip_conf(h)
}
pub(super) fn cip(c: &Option<IpConf>) -> String {
    c.as_ref().and_then(|c| c.ip.clone()).unwrap_or_default()
}
pub(super) fn cmask(c: &IpConf) -> String {
    c.mask.clone().unwrap_or_default()
}
pub(super) fn splitargs(line: &str) -> Vec<String> {
    line.split_whitespace().map(|s| s.to_string()).collect()
}
pub(super) fn nic_up(st: &mut State, id: &str) -> bool {
    let d = S::get(st);
    d.eps.get(&format!("E:{}:nic", id)).map(|e| e.up).unwrap_or(false)
}
pub(super) fn fe80(h: &Device) -> String {
    let m = h.mac();
    format!("fe80::{}:{}:{}:1", &m[0..4], &m[4..8], &m[8..12])
}
/// Klockan i spelet (dagen börjar 08:00)
pub(super) fn clock(st: &State) -> String {
    let s = 8 * 3600 + st.time.floor() as i64;
    format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60)
}
/// Math.round
pub(super) fn jsr(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}
/// parseInt(s, 10)
pub(super) fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let mut chars = t.chars().peekable();
    let mut out = String::new();
    if let Some(&c) = chars.peek() {
        if c == '-' || c == '+' {
            out.push(c);
            chars.next();
        }
    }
    let mut any = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            out.push(c);
            any = true;
            chars.next();
        } else {
            break;
        }
    }
    if any { out.parse().ok() } else { None }
}
/// parseInt(s) || dflt (0 räknas som falskt, som i JS)
pub(super) fn pint_or(s: Option<&String>, dflt: i64) -> i64 {
    s.and_then(|x| parse_int(x)).filter(|v| *v != 0).unwrap_or(dflt)
}
pub(super) fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}
pub(super) fn starts_ci(s: &str, p: &str) -> bool {
    s.len() >= p.len() && s.is_char_boundary(p.len()) && s[..p.len()].eq_ignore_ascii_case(p)
}
/// replace(/needle/gi, with)
pub(super) fn replace_ci(s: &str, needle: &str, with: &str) -> String {
    let ls = s.to_lowercase();
    let ln = needle.to_lowercase();
    if ls.len() != s.len() {
        return s.replace(needle, with);
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = ls[i..].find(&ln) {
        out.push_str(&s[i..i + p]);
        out.push_str(with);
        i += p + ln.len();
    }
    out.push_str(&s[i..]);
    out
}
pub(super) fn mac_of(h: &Device) -> String {
    h.mac()
}

// ------------------------------------------------------------------ Webb (kapitel 10)

pub(super) struct Url {
    pub host: String,
    pub port: i64,
}

/// Samma som regexen i parseUrl (scheme valfritt, host, :port, /sökväg)
pub(super) fn parse_url(u: &str) -> Option<Url> {
    if u.is_empty() || u.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let mut rest = u;
    let mut https = false;
    if starts_ci(rest, "https://") {
        https = true;
        rest = &rest[8..];
    } else if starts_ci(rest, "http://") {
        rest = &rest[7..];
    }
    let host_end = rest.find(|c| c == '/' || c == ':').unwrap_or(rest.len());
    if host_end == 0 {
        return None;
    }
    let host = &rest[..host_end];
    let mut rest = &rest[host_end..];
    let mut port = None;
    if rest.starts_with(':') {
        let digits: String = rest[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        port = digits.parse::<i64>().ok();
        rest = &rest[1 + digits.len()..];
    }
    if !rest.is_empty() && !rest.starts_with('/') {
        return None;
    }
    Some(Url { host: host.to_string(), port: port.unwrap_or(if https { 443 } else { 80 }) })
}

/// Gemensam curl: (text, fördröjning)
pub(super) fn curl_text(st: &mut State, host: &str, url: &str, head_only: bool) -> (String, u32) {
    let u = match parse_url(url) {
        Some(u) => u,
        None => return ("curl: (3) URL using bad/illegal format or missing URL".into(), 50),
    };
    let mut ip = u.host.clone();
    if !U::is_ip(&ip) {
        let rr = S::resolve(st, host, &u.host);
        if let Some(e) = &rr.error {
            return (format!("curl: (6) Could not resolve host: {}", u.host), if e == "timeout" { 2500 } else { 200 });
        }
        ip = rr.ip.unwrap_or_default();
    }
    let h = S::http_get(st, host, &ip, u.port);
    if !h.ok && h.code == 0 {
        if h.res.reason.as_deref() == Some("refused") {
            return (format!("curl: (7) Failed to connect to {} port {} after 2 ms: Couldn't connect to server", u.host, u.port), 200);
        }
        return (format!("curl: (28) Failed to connect to {} port {} after 10002 ms: Timeout was reached", u.host, u.port), 3500);
    }
    // Fulla paket genom en tunnel utan adjust-mss fastnar efter handskakningen
    let big = S::big_transfer(st, host, &ip, u.port);
    if !big.ok && big.stage.as_deref() == Some("transfer") {
        return ("curl: (28) Operation timed out after 30000 milliseconds with 0 out of 48213 bytes received".into(), 4500);
    }
    let mut inet: Option<String> = S::internet_host(&ip).map(|x| x.0.to_string());
    if h.code == 503 {
        return (
            if head_only {
                "HTTP/1.1 503 Service Unavailable\nServer: LB-Nordvik\nContent-Length: 107".into()
            } else {
                "<html><body><h1>503 Service Unavailable</h1>\nNo server is available to handle this request.\n</body></html>".into()
            },
            400,
        );
    }
    if h.code == 502 {
        return (
            if head_only {
                "HTTP/1.1 502 Bad Gateway\nServer: LB-Nordvik\nContent-Length: 107".into()
            } else {
                "<html><body><h1>502 Bad Gateway</h1>\nThe server returned an invalid or incomplete response.\n</body></html>".into()
            },
            1200,
        );
    }
    let hs: Option<String> = match &h.server {
        Some(s) if s.starts_with("INTERNET:") => None,
        Some(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    if inet.is_none() {
        if let Some(s) = &h.server {
            if let Some(rest) = s.strip_prefix("INTERNET:") {
                inet = S::internet_host(rest).map(|x| x.0.to_string());
            }
        }
    }
    let server = if let Some(hs) = hs {
        st.devices.get(&hs).map(|d| d.label.clone()).unwrap_or(hs)
    } else if let Some(i) = inet {
        i
    } else {
        u.host.clone()
    };
    if head_only {
        return (
            format!(
                "HTTP/1.1 200 OK\nServer: {}\nContent-Type: text/html; charset=utf-8{}",
                if h.member.is_some() { "nginx/1.24.0" } else { "Apache" },
                h.member.as_ref().map(|m| format!("\nX-Backend: {}", m.name)).unwrap_or_default()
            ),
            250,
        );
    }
    if let Some(m) = &h.member {
        return (
            format!(
                "<!doctype html>\n<title>Tidrapport – Nordvik</title>\n<h1>Tidrapport</h1>\n<p>Vecka 40 · inloggad via SSO</p>\n<!-- betjänad av {} ({}) -->",
                m.name, m.ip
            ),
            300,
        );
    }
    (format!("<!doctype html>\n<html><head><title>{0}</title></head>\n<body><h1>{0}</h1></body></html>", server), 300)
}

// ------------------------------------------------------------------ grep / findstr efter |

pub(super) struct PipeSpec {
    pub left: String,
    pub pat: String,
    pub i: bool,
    pub v: bool,
    pub count: bool,
}

/// `<kommando> | grep|findstr|egrep <mönster>`
pub(super) fn split_pipe(line: &str) -> Option<PipeSpec> {
    let bytes: Vec<(usize, char)> = line.char_indices().collect();
    for (k, &(pos, c)) in bytes.iter().enumerate() {
        if c != '|' || k == 0 || !bytes[k - 1].1.is_whitespace() {
            continue;
        }
        let after = &line[pos + 1..];
        let t = after.trim_start();
        let mut kw = None;
        for w in ["grep", "findstr", "egrep"] {
            if starts_ci(t, w) {
                let rest = &t[w.len()..];
                if rest.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
                    kw = Some((w, rest));
                    break;
                }
            }
        }
        // "grep" matchar före "egrep" bara om ordet börjar rätt; findstr/grep/egrep är disjunkta i början
        if let Some((w, rest)) = kw {
            let left = line[..bytes[k - 1].0].to_string();
            let mut o = PipeSpec { left, pat: String::new(), i: false, v: false, count: false };
            let mut pat: Vec<String> = vec![];
            for x in rest.trim().split_whitespace() {
                let lx = x.to_lowercase();
                match lx.as_str() {
                    "-i" | "/i" => o.i = true,
                    "-v" | "/v" => o.v = true,
                    "-c" | "/c" => o.count = true,
                    "-e" => {}
                    _ => {
                        let mut s = x.to_string();
                        if s.starts_with('"') || s.starts_with('\'') {
                            s.remove(0);
                        }
                        if s.ends_with('"') || s.ends_with('\'') {
                            s.pop();
                        }
                        pat.push(s);
                    }
                }
            }
            if w.eq_ignore_ascii_case("findstr") {
                let joined = pat.join("|");
                pat = vec![joined.split_whitespace().collect::<Vec<_>>().join("|")];
                // replace(/\s+/g,'|') på hela strängen
                let re_joined = pat[0].clone();
                pat = vec![re_joined];
            }
            o.pat = pat.join(" ");
            return Some(o);
        }
    }
    None
}

pub(super) fn pipe_filter(res: CmdResult, p: &PipeSpec) -> CmdResult {
    let mut text = res.out.clone();
    for c in &res.stream {
        text.push_str(&c.text);
    }
    let esc: String = p.pat.replace('_', "\\_");
    let (pat, case) = if p.i { (esc.to_lowercase(), true) } else { (esc, false) };
    let re = U::MiniRe::new(&pat);
    let lines: Vec<&str> = text.split('\n').filter(|l| p.count || !l.is_empty()).collect();
    let hit: Vec<&str> = lines
        .into_iter()
        .filter(|l| {
            let m = if case { re.is_match(&l.to_lowercase()) } else { re.is_match(l) };
            m != p.v
        })
        .collect();
    let total: u32 = if !res.stream.is_empty() { res.stream.iter().map(|c| c.delay).sum() } else { res.delay };
    let mut out = CmdResult::delayed(if p.count { hit.len().to_string() } else { hit.join("\n") }, total.min(3000));
    out.push = res.push;
    out.close = res.close;
    out.clear = res.clear;
    out
}

// ------------------------------------------------------------------ Förslag och komplettering

fn lev(a: &str, b: &str) -> usize {
    super::levenshtein(a, b)
}
/// NV.suggestCmd i host_more.js
pub(super) fn suggest(word: &str, pool: &[&str]) -> Option<String> {
    let n = word.chars().count();
    let mut bd = if n <= 2 { 1 } else if n <= 4 { 2 } else { 3 };
    let mut best = None;
    for w in pool {
        let d = lev(&word.to_lowercase(), w);
        if d < bd && d < n {
            bd = d;
            best = Some(w.to_string());
        }
    }
    best
}

const ARGS: &[&str] = &[
    "192.168.1.1", "192.168.1.10", "192.168.1.13", "192.168.1.193", "192.168.2.1", "192.168.2.193", "198.51.100.80",
    "drift@192.168.1.193", "drift@192.168.2.193", "admin@192.168.1.13", "admin@192.168.1.196", "http://tid/",
    "/dev/ttyUSB0", "filserver", "www.example.com",
];
const LINUX_CMDS: &[&str] = &[
    "ssh", "screen", "ping", "traceroute", "tracepath", "curl", "wget", "nslookup", "dig", "resolvectl", "ip", "telnet", "nc",
    "man", "history", "clear", "exit", "echo", "help", "hostname", "dmesg", "ls", "neofetch", "fortune", "uptime", "date",
    "whoami", "sudo", "for",
];
const WIN_CMDS: &[&str] = &[
    "ipconfig", "ping", "tracert", "nslookup", "arp", "netsh", "curl", "copy", "route", "netstat", "getmac", "hostname", "whoami",
    "systeminfo", "test-netconnection", "cls", "exit", "help", "ver", "date", "time",
];
pub(super) const WIN_EXTRA: &[&str] = &[
    "get-history", "doskey", "get-netneighbor", "clear-dnsclientcache", "get-nettcpconnection", "get-date", "get-command",
    "nbtstat", "get-netroute", "get-dnsclientserveraddress", "pathping", "test-connection", "resolve-dnsname",
    "get-netipaddress", "get-netipconfiguration", "get-netadapter", "net", "w32tm", "dir", "cd", "type", "tasklist", "set",
];
pub(super) const LIN_EXTRA: &[&str] = &[
    "wget", "free", "df", "ps", "lsof", "last", "env", "ping6", "ssh-copy-id", "scp", "tcpdump", "iperf3", "nmap", "mtr", "ss",
    "netstat", "route", "ethtool", "nmcli", "dhclient", "arping", "ssh-keygen", "hostnamectl", "pwd",
];

fn complete_from(words: &[&str], line: &str) -> Option<String> {
    // m = /^(.*?)(\S*)$/
    let cut = line.rfind(char::is_whitespace).map(|i| i + line[i..].chars().next().unwrap().len_utf8()).unwrap_or(0);
    let (head, last) = line.split_at(cut);
    if last.is_empty() {
        return None;
    }
    let pool: &[&str] = if !head.trim().is_empty() { ARGS } else { words };
    let hits: Vec<&str> = pool.iter().copied().filter(|w| w.starts_with(last) && *w != last).collect();
    if hits.is_empty() {
        return None;
    }
    let mut common: String = hits[0].to_string();
    for h in &hits[1..] {
        let n = common.chars().zip(h.chars()).take_while(|(a, b)| a == b).count();
        common = common.chars().take(n).collect();
    }
    if common.chars().count() <= last.chars().count() {
        return None;
    }
    Some(format!("{}{}{}", head, common, if hits.len() == 1 { " " } else { "" }))
}

// ------------------------------------------------------------------ Inställningar från dialogrutan

/// Inställningar från nätverksdialogen (applyIpv4 i JS).
pub struct Ipv4Settings {
    pub dhcp: bool,
    pub ip: String,
    pub mask: String,
    pub gw: Option<String>,
    pub dns: Option<String>,
}

/// Tillämpar IPv4-inställningar på en värd. Spelet räknar själv kommandot "[inställningar]"
/// (NV.onHostCommand i JS) om det ska räknas.
pub fn apply_ipv4(st: &mut State, host: &str, s: &Ipv4Settings) {
    if s.dhcp {
        st.dev_mut(host).nic.as_mut().unwrap().dhcp = true;
        S::touch(st);
        S::dhcp(st, host);
    } else {
        let d = st.dev_mut(host);
        let old_dns = d.nic().static_.as_ref().map(|x| x.dns.clone()).unwrap_or_default();
        let nic = d.nic.as_mut().unwrap();
        nic.dhcp = false;
        nic.static_ = Some(StaticIp {
            ip: s.ip.clone(),
            mask: s.mask.clone(),
            gw: s.gw.clone().filter(|g| !g.is_empty()),
            dns: match &s.dns {
                Some(x) if !x.is_empty() => vec![x.clone()],
                _ => old_dns,
            },
        });
        d.lease = None;
        S::touch(st);
    }
}

// ------------------------------------------------------------------ Cli

impl Cli for HostSession {
    fn prompt_text(&self, st: &State) -> String {
        if let Some(p) = &self.pending {
            return p.prompt.clone();
        }
        if self.linux { "tekniker@laptop:~$ ".into() } else { self.win_prompt() }
    }
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        if let Some(p) = self.pending.take() {
            return self.answer(st, p, line);
        }
        if !line.trim().is_empty() {
            self.events.push(format!("cmd:{}", line));
        }
        if self.linux { self.lin_top(st, line) } else { self.win_layer(st, line) }
    }
    fn help(&mut self, st: &mut State, line: &str) -> String {
        if self.pending.is_some() {
            return String::new();
        }
        if self.linux { self.lin_layer(st, "help").out } else { self.win_layer(st, "help").out }
    }
    fn complete(&mut self, st: &mut State, line: &str) -> Option<String> {
        if self.linux {
            if self.pending.is_some() {
                return None;
            }
            if let Some(x) = complete_from(LINUX_CMDS, line) {
                return Some(x);
            }
            if line.is_empty() || line.contains(char::is_whitespace) {
                return None;
            }
            let hits: Vec<&&str> = LIN_EXTRA.iter().filter(|w| w.starts_with(line)).collect();
            if hits.len() == 1 { Some(format!("{} ", hits[0])) } else { None }
        } else {
            if let Some(x) = complete_from(WIN_CMDS, line) {
                return Some(x);
            }
            if line.is_empty() || line.contains(char::is_whitespace) {
                return None;
            }
            let l = line.to_lowercase();
            let hits: Vec<&&str> = WIN_EXTRA.iter().filter(|w| w.starts_with(&l)).collect();
            if hits.len() == 1 { Some(format!("{} ", hits[0])) } else { None }
        }
    }
    fn secret_input(&self) -> bool {
        self.pending.as_ref().map(|p| p.secret).unwrap_or(false)
    }
    fn closed(&self) -> bool {
        self.closed
    }
    fn dev_id(&self) -> &str {
        &self.dev
    }
}

/// Tips efter ett felaktigt lösenord (i riktiga livet får man förstås inget sådant)
pub(super) fn pw_tip(c: &IosConfig, user: &str, line: &Line) -> String {
    let u = c.users.get(user);
    if !line.login_local {
        return if line.password.as_deref().map(|p| !p.is_empty()).unwrap_or(false) {
            "\n💡 Linjen använder lösenordet som står under line vty i konfigurationen.".into()
        } else {
            String::new()
        };
    }
    match u {
        None => {
            let names: Vec<&String> = c.users.keys().collect();
            format!(
                "\n💡 Det finns inget konto som heter {}.{}",
                user,
                if !names.is_empty() {
                    format!(" Konton: {}.", names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
                } else {
                    " Skapa ett med username <namn> privilege 15 secret <lösenord>.".into()
                }
            )
        }
        Some(u) if u.by_game => format!("\n💡 Lösenordet för {} är {} (står på lappen vid laptopen).", user, u.plain),
        Some(u) => format!("\n💡 Det är lösenordet du själv satte med username {} ... secret: {}", user, u.plain),
    }
}

impl HostSession {
    pub(super) fn set_pending(&mut self, prompt: impl Into<String>, secret: bool, kind: PendKind) {
        self.pending = Some(Pending { prompt: prompt.into(), secret, kind });
    }
    pub(super) fn push_res(p: PushReq) -> CmdResult {
        CmdResult { push: Some(p), ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::sim::State;

    fn st_with(os: Os, id: &str, dhcp: bool, ip: Option<(&str, &str, Option<&str>, Vec<&str>)>) -> State {
        let mut devices = IndexMap::new();
        let mut o = HostOpts::new("Test");
        o.dhcp = dhcp;
        o.ip = ip;
        o.user = Some("Åsa");
        devices.insert(id.to_string(), make_host(id, os, o));
        State::new(Topology { devices, links: vec![] })
    }

    #[test]
    fn windows_help_och_banner() {
        let mut st = st_with(Os::Windows, "PC-Anna", true, None);
        let mut s = HostSession::new(&st, "PC-Anna");
        assert!(s.banner(&st).starts_with("Microsoft Windows"));
        assert_eq!(s.prompt_text(&st), "C:\\Users\\asa>");
        let r = s.handle(&mut st, "help");
        assert!(r.out.contains("ipconfig [/all | /release | /renew]"));
        assert!(r.out.contains("Fler kommandon:"));
        assert_eq!(s.events, vec!["cmd:help".to_string()]);
        let r = s.handle(&mut st, "ipcnofig");
        assert!(r.out.contains("känns inte igen"));
        assert!(r.out.contains("Menade du ipconfig?"));
        assert!(s.handle(&mut st, "cls").clear);
        assert!(s.handle(&mut st, "exit").close);
        assert_eq!(s.handle(&mut st, "hostname").out, "PC-Anna");
        assert_eq!(s.handle(&mut st, "echo hej %USERNAME%").out, "hej asa");
    }

    #[test]
    fn pipe_findstr_pa_help() {
        let mut st = st_with(Os::Windows, "PC-Anna", true, None);
        let mut s = HostSession::new(&st, "PC-Anna");
        let r = s.handle(&mut st, "help | findstr tracert");
        assert_eq!(r.out.trim(), "tracert <adress|namn>");
        let r = s.handle(&mut st, "help | findstr /c ping");
        assert!(r.out.parse::<usize>().unwrap() >= 1);
    }

    #[test]
    fn linux_help_historik_och_pipe() {
        let mut st = st_with(Os::Linux, "Tekniker", false, Some(("192.168.1.50", "255.255.255.0", Some("192.168.1.1"), vec!["192.168.1.1"])));
        let mut s = HostSession::new(&st, "Tekniker");
        assert_eq!(s.prompt_text(&st), "tekniker@laptop:~$ ");
        let r = s.handle(&mut st, "help");
        assert!(r.out.starts_with("Kommandon på laptopen:"));
        assert!(r.out.contains("Fler kommandon:"));
        assert_eq!(s.handle(&mut st, "whoami").out, "tekniker");
        let h = s.handle(&mut st, "history").out;
        assert!(h.contains("    1  help"), "{}", h);
        assert!(h.contains("    3  history"), "{}", h);
        // !! kör om
        let r = s.handle(&mut st, "!!");
        assert!(r.out.starts_with("history\n"));
        let r = s.handle(&mut st, "help | grep -i fortune");
        assert!(r.out.contains("fortune"));
        assert!(!r.out.contains("screen"));
        let r = s.handle(&mut st, "sudo scren /dev/ttyUSB0");
        assert!(r.out.contains("command not found"));
        assert!(r.out.contains("Menade du screen?"));
        assert_eq!(s.handle(&mut st, "pwd").out, "/home/tekniker");
    }

    #[test]
    fn screen_kraver_konsolkabel() {
        let mut st = st_with(Os::Linux, "Tekniker", false, None);
        let mut s = HostSession::new(&st, "Tekniker");
        let r = s.handle(&mut st, "sudo screen /dev/ttyUSB0 9600");
        assert!(r.out.contains("Cannot exec"));
        assert_eq!(s.handle(&mut st, "ls /dev/ttyUSB*").out, "ls: cannot access '/dev/ttyUSB*': No such file or directory");
        s.console_target = Some("SW1".into());
        let r = s.handle(&mut st, "sudo screen /dev/ttyUSB0 9600");
        match r.push {
            Some(PushReq::Serial { target, speed }) => {
                assert_eq!(target, "SW1");
                assert_eq!(speed, 9600);
            }
            _ => panic!("ingen push"),
        }
        assert_eq!(s.handle(&mut st, "ls /dev/ttyUSB*").out, "/dev/ttyUSB0");
    }

    #[test]
    fn for_loop_syntax_och_komplettering() {
        let mut st = st_with(Os::Linux, "Tekniker", false, None);
        let mut s = HostSession::new(&st, "Tekniker");
        assert!(s.handle(&mut st, "for i in 1 2; curl x").out.starts_with("bash: syntax:"));
        assert_eq!(s.complete(&mut st, "trace"), None);
        assert_eq!(s.complete(&mut st, "tcpd"), Some("tcpdump ".into()));
        assert_eq!(s.complete(&mut st, "screen /dev/tty"), Some("screen /dev/ttyUSB0 ".into()));
    }

    #[test]
    fn url_och_pipe_parsning() {
        let u = parse_url("http://tid/").unwrap();
        assert_eq!((u.host.as_str(), u.port), ("tid", 80));
        let u = parse_url("https://www.example.com:8443/x").unwrap();
        assert_eq!((u.host.as_str(), u.port), ("www.example.com", 8443));
        assert!(parse_url("http://a b").is_none());
        let p = split_pipe("ping x | grep -v time").unwrap();
        assert_eq!(p.left, "ping x");
        assert!(p.v);
        assert_eq!(p.pat, "time");
        assert!(split_pipe("ping x").is_none());
        assert_eq!(parse_int("12abc"), Some(12));
        assert_eq!(parse_int("abc"), None);
    }
}
