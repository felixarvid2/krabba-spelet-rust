//! Windows-terminalen: grundkommandona (host.js, WinShell).
use super::host::*;
use super::CmdResult;
use crate::net::sim::{self as S, PingOpts, State};
use crate::net::util as U;

impl HostSession {
    pub(super) fn resolve_name(&self, st: &mut State, name: &str) -> (Option<String>, Option<String>) {
        // (ip, fel)
        if U::is_ip(name) {
            return (Some(name.to_string()), None);
        }
        let rr = S::resolve(st, &self.dev, name);
        (rr.ip, rr.error)
    }

    /// Grundkommandona (WinShell.prototype.handle). Pipen hanteras redan i win_layer.
    pub(super) fn win_base(&mut self, st: &mut State, line: &str) -> CmdResult {
        if let Some(pp) = split_pipe(line) {
            let left = self.win_layer(st, &pp.left);
            return pipe_filter(left, &pp);
        }
        let a = splitargs(line);
        if a.is_empty() {
            return r("");
        }
        let cmd = a[0].to_lowercase();
        let id = self.dev.clone();
        match cmd.as_str() {
            "ipconfig" => return r(self.ipconfig(st, &a[1..])),
            "ping" => return self.win_ping(st, &a[1..]),
            "tracert" => return self.tracert(st, &a[1..]),
            "arp" => return r(self.win_arp(st, &a[1..])),
            "nslookup" => return r(self.nslookup(st, &a[1..])),
            "hostname" => return r(id),
            "echo" => {
                let t = line.trim();
                let body: String = t.chars().skip(5).collect();
                let body = replace_ci(&body, "%USERNAME%", &self.user);
                let body = replace_ci(&body, "%COMPUTERNAME%", &id.to_uppercase());
                return r(if body.is_empty() { "ECHO är aktiverat.".to_string() } else { body });
            }
            "cls" => return CmdResult { clear: true, ..Default::default() },
            "exit" => return CmdResult { close: true, ..Default::default() },
            "ver" => return r("\nMicrosoft Windows [Version 10.0.19045.4894]\n"),
            "whoami" => return r(format!("nordvik\\{}", self.user)),
            "date" => return r("Aktuellt datum: 2026-09-29"),
            "time" => return r(format!("Aktuell tid: {}", clock(st))),
            "systeminfo" => {
                let h = self.me(st);
                let dhcp = h.nic.as_ref().map(|n| n.dhcp).unwrap_or(false);
                let ip = conf(h).and_then(|c| c.ip).unwrap_or_else(|| "–".into());
                return r(vec![
                    String::new(),
                    format!("Värdnamn:                  {}", id.to_uppercase()),
                    "OS-namn:                   Microsoft Windows 10 Pro".into(),
                    "Registrerad ägare:         Nordvik AB".into(),
                    "Domän:                     nordvik.local".into(),
                    "Nätverkskort:              1 NIC(s) installerade.".into(),
                    "                           [01]: Intel(R) Ethernet Connection".into(),
                    format!("                                 DHCP aktiverat: {}", if dhcp { "Ja" } else { "Nej" }),
                    format!("                                 IP-adresser: {}", ip),
                    String::new(),
                ]
                .join("\n"));
            }
            "route" => return r(self.route_print(st)),
            "netstat" => {
                return r(if line.to_lowercase().contains("-r") {
                    self.route_print(st)
                } else {
                    "Aktiva anslutningar\n\n  Proto  Lokal adress           Utländsk adress        Tillstånd".to_string()
                })
            }
            "test-netconnection" | "tnc" => return self.tnc(st, &a[1..]),
            "getmac" => {
                let mac = U::mac_dash(&self.me(st).mac());
                return r(format!("\nFysisk adress       Transportnamn\n=================== ==========================================================\n{}   \\Device\\Tcpip_{{4E2B9A1C-7D11-4F2E-9B3A-1C5E7D9F0A21}}", mac));
            }
            "netsh" => return r(self.netsh(st, line)),
            "curl" | "curl.exe" | "iwr" | "invoke-webrequest" => {
                let url = a[1..].iter().find(|x| !x.starts_with('-'));
                let Some(url) = url else { return r("curl: try 'curl --help' for more information") };
                let (t, d) = curl_text(st, &id, url, a.iter().any(|x| x == "-I"));
                return rd(t, d);
            }
            "copy" | "xcopy" | "robocopy" => return self.win_copy(st, &a[1..]),
            "help" | "/?" => {
                return r("Kommandon som fungerar här:\n  ipconfig [/all | /release | /renew]\n  ping <adress|namn> [-n antal] [-l storlek] [-f]\n  tracert <adress|namn>\n  arp -a\n  nslookup <namn>\n  curl http://<adress|namn>/      (webbsida, t.ex. tidrapporten)\n  copy \\\\<server>\\<share>\\<fil> .   (hämta en fil från en filserver)\n  netsh interface ip set address \"Ethernet\" static <ip> <mask> <gateway>\n  netsh interface ip set address \"Ethernet\" dhcp\n  netsh interface ip set dns \"Ethernet\" static <ip>\n  hostname, getmac, whoami, ver, date, time, systeminfo, cls, exit");
            }
            _ => {}
        }
        r(format!("'{}' känns inte igen som ett internt eller externt kommando,\nkörbart program eller kommandofil.", a[0]))
    }

    fn route_print(&self, st: &State) -> String {
        let h = self.me(st);
        let c = conf(h);
        let mut o: Vec<String> = vec![
            "===========================================================================".into(),
            "Gränssnittslista".into(),
            format!(" 12...{} ......Intel(R) Ethernet Connection I219-LM", U::mac_dash(&h.mac()).replace('-', " ")),
            "  1...........................Software Loopback Interface 1".into(),
            "===========================================================================".into(),
            String::new(),
            "IPv4-routningstabell".into(),
            "===========================================================================".into(),
            "Aktiva vägar:".into(),
            "Nätverksmål           Nätmask         Gateway        Gränssnitt  Mått".into(),
        ];
        if let Some(c) = &c {
            if let Some(ip) = c.ip.as_ref().filter(|x| !x.is_empty()) {
                let mask = cmask(c);
                if let Some(gw) = c.gw.as_ref().filter(|x| !x.is_empty()) {
                    o.push(format!("{}{}{}{}25", U::pad("          0.0.0.0", 22), U::pad("0.0.0.0", 16), U::pad(gw, 15), U::pad(ip, 12)));
                }
                o.push(format!("{}{}{}{}281", U::pad(format!("      {}", U::network(ip, &mask)), 22), U::pad(&mask, 16), U::pad("På länk", 15), U::pad(ip, 12)));
                o.push(format!("{}{}{}{}281", U::pad(format!("      {}", ip), 22), U::pad("255.255.255.255", 16), U::pad("På länk", 15), U::pad(ip, 12)));
            }
        }
        o.push(format!("{}{}{}{}331", U::pad("        127.0.0.0", 22), U::pad("255.0.0.0", 16), U::pad("På länk", 15), U::pad("127.0.0.1", 12)));
        o.push("===========================================================================".into());
        o.push("Beständiga vägar:".into());
        o.push("  Inga".into());
        o.join("\n")
    }

    fn tnc(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let mut host: Option<String> = None;
        let mut port: Option<i64> = None;
        let mut i = 0;
        while i < args.len() {
            if args[i].eq_ignore_ascii_case("-port") {
                port = args.get(i + 1).and_then(|x| parse_int(x));
                i += 1;
            } else if !args[i].starts_with('-') {
                host = Some(args[i].clone());
            }
            i += 1;
        }
        let Some(host) = host else { return r("Test-NetConnection <värd> [-Port <nummer>]") };
        let (ip, err) = self.resolve_name(st, &host);
        if err.is_some() {
            return rd(format!("WARNING: Name resolution of {} failed", host), 800);
        }
        let ip = ip.unwrap_or_default();
        let port = port.filter(|p| *p != 0);
        let p = match port {
            Some(pt) => S::ping(st, &self.dev, &ip, PingOpts::tcp(pt)),
            None => S::ping(st, &self.dev, &ip, PingOpts::default()),
        };
        let c = conf(self.me(st));
        let mut o = vec![String::new(), format!("ComputerName     : {}", host), format!("RemoteAddress    : {}", ip)];
        if let Some(pt) = port {
            o.push(format!("RemotePort       : {}", pt));
        }
        o.push("InterfaceAlias   : Ethernet".into());
        o.push(format!("SourceAddress    : {}", cip(&c)));
        if port.is_some() {
            o.push(format!("TcpTestSucceeded : {}", if p.ok { "True" } else { "False" }));
        } else {
            o.push(format!("PingSucceeded    : {}", if p.ok { "True" } else { "False" }));
        }
        rd(o.join("\n"), if p.ok { 400 } else { 2500 })
    }

    pub(super) fn ipconfig(&mut self, st: &mut State, args: &[String]) -> String {
        let id = self.dev.clone();
        let flag = args.get(0).map(|x| x.to_lowercase()).unwrap_or_default();
        let dhcp = self.me(st).nic.as_ref().map(|n| n.dhcp).unwrap_or(false);
        if flag == "/flushdns" {
            return "\nWindows IP-konfiguration\n\nDNS-matchningscachen har tömts.".into();
        }
        if flag == "/displaydns" {
            return "\nWindows IP-konfiguration\n\n    filserver.nordvik.example\n    ----------------------------------------\n    Postnamn . . . . . . . : filserver.nordvik.example\n    Posttyp  . . . . . . . : 1\n    Data-sektion . . . . . : Svar\n    A-post (värd)  . . . . : 192.168.1.10".into();
        }
        if flag == "/release" {
            if !dhcp {
                return "\nWindows IP-konfiguration\n\nDet gick inte att utföra åtgärden på gränssnittet Ethernet eftersom DHCP inte är aktiverat.".into();
            }
            S::release(st, &id);
            return format!("\nWindows IP-konfiguration\n\nEthernet-kort Ethernet:\n\n   Anslutningsspecifikt DNS-suffix. . :\n   Länklokal IPv6-adress . . . . . . . : {}%12\n   Standardgateway . . . . . . . . . . :", fe80(self.me(st)));
        }
        if flag == "/renew" {
            if !dhcp {
                return "\nWindows IP-konfiguration\n\nDet gick inte att utföra åtgärden på gränssnittet Ethernet eftersom DHCP inte är aktiverat.".into();
            }
            if !nic_up(st, &id) {
                S::dhcp(st, &id);
                return "\nWindows IP-konfiguration\n\nDet går inte att utföra någon åtgärd på Ethernet när mediet är frånkopplat.".into();
            }
            let l = S::dhcp(st, &id);
            if l.apipa {
                return "\nWindows IP-konfiguration\n\nEtt fel uppstod när gränssnittet Ethernet förnyades: Det gick inte att kontakta\nDHCP-servern. Tidsgränsen för begäran har överskridits.".into();
            }
            return self.ipconfig(st, &[]);
        }
        let all = flag == "/all";
        let up = nic_up(st, &id);
        let h = self.me(st);
        let c = conf(h);
        let lease = h.lease.clone();
        let lease_nolink = lease.as_ref().map(|l| l.nolink).unwrap_or(false);
        let mac = U::mac_dash(&h.mac());
        let wireless = h.wireless;
        let fe = fe80(h);
        let mut o: Vec<String> = vec![String::new(), "Windows IP-konfiguration".into(), String::new()];
        if all {
            o.push(format!("   Värdnamn . . . . . . . . . . . . . : {}", id));
            o.push("   Primärt DNS-suffix . . . . . . . . :".into());
            o.push("   Nodtyp . . . . . . . . . . . . . . : Hybrid".into());
            o.push("   IP-routning aktiverad. . . . . . . : Nej".into());
            o.push("   WINS-proxy aktiverad . . . . . . . : Nej".into());
            o.push(String::new());
        }
        o.push(String::new());
        o.push(if wireless { "Trådlöst nätverkskort Wi-Fi:".into() } else { "Ethernet-kort Ethernet:".into() });
        o.push(String::new());
        let c_apipa = c.as_ref().map(|c| c.apipa).unwrap_or(false);
        if !up && !(c.is_some() && c_apipa && !lease_nolink && dhcp) {
            o.push("   Mediastatus . . . . . . . . . . . . : Media frånkopplat".into());
            o.push("   Anslutningsspecifikt DNS-suffix. . :".into());
            if all {
                o.push("   Beskrivning . . . . . . . . . . . . : Intel(R) Ethernet Connection I219-LM".into());
                o.push(format!("   Fysisk adress . . . . . . . . . . . : {}", mac));
                o.push(format!("   DHCP aktiverat. . . . . . . . . . . : {}", if dhcp { "Ja" } else { "Nej" }));
                o.push("   Autokonfiguration aktiverad . . . . : Ja".into());
            }
            return o.join("\n");
        }
        o.push(format!("   Anslutningsspecifikt DNS-suffix. . : {}", if c.is_some() && !c_apipa && dhcp { "nordvik.example" } else { "" }));
        if all {
            o.push(format!("   Beskrivning . . . . . . . . . . . . : {}", if wireless { "Intel(R) Wi-Fi 6 AX201 160MHz" } else { "Intel(R) Ethernet Connection I219-LM" }));
            o.push(format!("   Fysisk adress . . . . . . . . . . . : {}", mac));
            o.push(format!("   DHCP aktiverat. . . . . . . . . . . : {}", if dhcp { "Ja" } else { "Nej" }));
            o.push("   Autokonfiguration aktiverad . . . . : Ja".into());
        }
        o.push(format!("   Länklokal IPv6-adress . . . . . . . : {}%12{}", fe, if all { "(Föredragen)" } else { "" }));
        let Some(c) = c else {
            o.push("   Standardgateway . . . . . . . . . . :".into());
            return o.join("\n");
        };
        let ip = c.ip.clone().unwrap_or_default();
        if c.apipa {
            o.push(format!("   Autokonfiguration av IPv4-adress. . : {}{}", ip, if all { "(Föredragen)" } else { "" }));
        } else {
            // Dubblettadress (c.dup i JS) finns inte i IpConf
            o.push(format!("   IPv4-adress . . . . . . . . . . . . : {}{}", ip, if all { "(Föredragen)" } else { "" }));
        }
        o.push(format!("   Nätmask . . . . . . . . . . . . . . : {}", cmask(&c)));
        if all && dhcp && !c.apipa {
            let d = U::hash(&id) % 10;
            o.push(format!("   Lån erhållet. . . . . . . . . . . . : den 29 september 2026 07:5{}:12", d));
            o.push(format!("   Lånet upphör att gälla. . . . . . . : den 30 september 2026 07:5{}:12", d));
        }
        o.push(format!("   Standardgateway . . . . . . . . . . : {}", c.gw.clone().unwrap_or_default()));
        if all {
            if dhcp && !c.apipa {
                o.push(format!("   DHCP-server . . . . . . . . . . . . : {}", lease.as_ref().and_then(|l| l.server.clone()).unwrap_or_default()));
            }
            if !c.dns.is_empty() {
                o.push(format!("   DNS-servrar . . . . . . . . . . . . : {}", c.dns[0]));
                for d in &c.dns[1..] {
                    o.push(format!("                                         {}", d));
                }
            } else {
                o.push("   DNS-servrar . . . . . . . . . . . . : fec0:0:0:ffff::1%1".into());
                o.push("                                         fec0:0:0:ffff::2%1".into());
            }
            o.push("   NetBIOS över TCP/IP . . . . . . . . : Aktiverat".into());
        }
        o.join("\n")
    }

    fn win_copy(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let src = args.get(0).cloned().unwrap_or_default();
        let parsed = src.strip_prefix("\\\\").and_then(|rest| {
            let mut it = rest.splitn(3, '\\');
            let (a, b, c) = (it.next()?, it.next()?, it.next()?);
            if a.is_empty() || b.is_empty() || c.is_empty() { None } else { Some(a.to_string()) }
        });
        let Some(server) = parsed else {
            return r("Syntaxen för kommandot är felaktig.\n(Exempel: copy \\\\filserver\\ritningar\\hyllplan.pdf .)");
        };
        let (ip, err) = self.resolve_name(st, &server);
        if err.is_some() {
            return rd("Det gick inte att hitta nätverkssökvägen.", 2500);
        }
        let ip = ip.unwrap_or_default();
        let t = S::big_transfer(st, &self.dev, &ip, 445);
        if !t.ok && t.stage.as_deref() == Some("connect") {
            if t.res.reason.as_deref() == Some("refused") {
                return rd("Det gick inte att hitta nätverkssökvägen.", 600);
            }
            return rd("Det gick inte att hitta nätverkssökvägen.", 3500);
        }
        if !t.ok {
            return rd("Det angivna nätverksnamnet är inte längre tillgängligt.\n        0 fil(er) kopierade.", 5000);
        }
        rd("        1 fil(er) kopierade.", 700)
    }

    pub(super) fn win_ping(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let id = self.dev.clone();
        let mut n: i64 = 4;
        let mut target: Option<String> = None;
        let mut size: i64 = 32;
        let mut df = false;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "-n" && i + 1 < args.len() {
                n = pint_or(args.get(i + 1), 4).min(20);
                i += 1;
            } else if args[i] == "-l" && i + 1 < args.len() {
                size = pint_or(args.get(i + 1), 32).clamp(0, 65500);
                i += 1;
            } else if args[i] == "-f" {
                df = true;
            } else if args[i] == "-t" {
                n = 8;
            } else if !args[i].starts_with('-') {
                target = Some(args[i].clone());
            }
            i += 1;
        }
        let Some(target) = target else { return r("\nSyntax: ping [-t] [-n antal] [-l storlek] [-f] mål\n") };
        let (ip, err) = self.resolve_name(st, &target);
        if let Some(e) = err {
            return rd(
                format!("Det gick inte att hitta värden {} med ping-begäran. Kontrollera namnet och försök igen.", target),
                if e == "timeout" { 2500 } else { 300 },
            );
        }
        let ip = ip.unwrap_or_default();
        let head = if U::is_ip(&target) {
            ip.clone()
        } else {
            format!("{} [{}]", if !target.contains('.') { format!("{}.nordvik.example", target) } else { target.clone() }, ip)
        };
        let me = conf(self.me(st));
        let intro = format!("\nSkickar ping-signal till {} med {} byte data:\n", head, size);
        let stat_zero = |ip: &str, n: i64| format!("\nPing-statistik för {}:\n    Paket: Skickade = {}, Mottagna = 0, Förlorade = {} (100 % förlust),", ip, n, n);
        // Det egna nätkortet har MTU 1500: 1472 byte data + 28 byte huvud
        if df && size > 1472 {
            let mut loc = vec![ck(intro, 100)];
            for _ in 0..n {
                loc.push(ck("Paketet måste fragmenteras men DF har angetts.\n", 200));
            }
            loc.push(ck(stat_zero(&ip, n), 50));
            return rs(loc);
        }
        let p = S::ping(st, &id, &ip, PingOpts { size: Some(size + 28), df, ..Default::default() });
        if !p.ok && p.reason.as_deref() == Some("frag") {
            let mut fr = vec![ck(intro, 100)];
            for _ in 0..n {
                fr.push(ck("Paketet måste fragmenteras men DF har angetts.\n", 400));
            }
            fr.push(ck(stat_zero(&ip, n), 50));
            return rs(fr);
        }
        let reason = p.reason.clone().unwrap_or_default();
        let mut stream = vec![ck(intro, 100)];
        let mut ok = 0i64;
        let mut lost_n = 0i64;
        let mut times: Vec<i64> = vec![];
        for _ in 0..n {
            let line;
            let delay;
            if p.ok && st.random() >= p.loss {
                let mut t = if !p.hops.is_empty() { 1 + p.hops.len() as i64 * 2 + (st.random() * 3.0).floor() as i64 } else { 0 };
                if p.loss > 0.0 {
                    t += 8 + (st.random() * 40.0).floor() as i64;
                }
                times.push(t);
                line = format!("Svar från {}: byte={} {} TTL={}", ip, size, if t < 1 { "tid<1 ms".to_string() } else { format!("tid={} ms", t) }, p.ttl);
                ok += 1;
                delay = 600;
            } else if !p.ok && (reason == "nolink" || reason == "noip" || reason == "nogw") {
                return rd("PING: överföringen misslyckades. Allmänt fel.", 200);
            } else if !p.ok && reason == "arp" && p.local {
                line = format!("Svar från {}: Målvärddatorn kan inte nås.", cip(&me));
                delay = 2600;
                lost_n += 1;
            } else if !p.ok && reason == "net-unreachable" && p.from_ip.is_some() {
                line = format!("Svar från {}: Målnätverket kan inte nås.", p.from_ip.clone().unwrap());
                delay = 300;
                lost_n += 1;
            } else if !p.ok && reason == "acl" && p.from_ip.is_some() {
                line = format!("Svar från {}: Målvärddatorn kan inte nås.", p.from_ip.clone().unwrap());
                delay = 300;
                lost_n += 1;
            } else if !p.ok && reason == "arp" && p.from_ip.is_some() && !p.local {
                line = format!("Svar från {}: Målvärddatorn kan inte nås.", p.from_ip.clone().unwrap());
                delay = 2600;
                lost_n += 1;
            } else {
                line = "Begäran gjorde time out.".to_string();
                delay = 3000;
                lost_n += 1;
            }
            stream.push(ck(format!("{}\n", line), delay));
        }
        let answered = ok + if p.ok { 0 } else if reason == "net-unreachable" || reason == "acl" || reason == "arp" { lost_n } else { 0 };
        let lost = n - answered;
        let mut tail = format!(
            "\nPing-statistik för {}:\n    Paket: Skickade = {}, Mottagna = {}, Förlorade = {} ({} % förlust),",
            ip, n, answered, lost, jsr(lost as f64 * 100.0 / n as f64)
        );
        if ok > 0 {
            let sum: i64 = times.iter().sum();
            tail.push_str(&format!(
                "\nUngefärlig tid för tur och retur i millisekunder:\n    Lägsta = {} ms, Högsta = {} ms, Medel = {} ms",
                times.iter().min().unwrap(),
                times.iter().max().unwrap(),
                jsr(sum as f64 / times.len() as f64)
            ));
        }
        stream.push(ck(tail, 50));
        rs(stream)
    }

    fn tracert(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let Some(target) = args.iter().find(|x| !x.starts_with('-')).cloned() else { return r("\nSyntax: tracert mål\n") };
        let (ip, err) = self.resolve_name(st, &target);
        if err.is_some() {
            return r(format!("Det gick inte att matcha målsystemnamnet {}.", target));
        }
        let ip = ip.unwrap_or_default();
        let (fwd, full) = S::traceroute(st, &self.dev, &ip, None);
        let mut stream = vec![ck(
            format!("\nSpårar väg till {}\növer högst 30 hopp:\n\n", if U::is_ip(&target) { ip.clone() } else { format!("{} [{}]", target, ip) }),
            200,
        )];
        let mut n = 1;
        for hp in &fwd.hops {
            stream.push(ck(format!("{}    <1 ms    <1 ms    <1 ms  {}\n", U::pad_l(n, 3), hp.ip), 500));
            n += 1;
        }
        if full.ok {
            if fwd.hops.last().map(|h| h.ip != ip).unwrap_or(true) {
                stream.push(ck(format!("{}     1 ms    <1 ms     1 ms  {}\n", U::pad_l(n, 3), ip), 500));
            }
            stream.push(ck("\nSpårningen är slutförd.", 100));
        } else {
            for _ in 0..4 {
                stream.push(ck(format!("{}     *        *        *     Begäran gjorde time out.\n", U::pad_l(n, 3)), 2200));
                n += 1;
            }
            stream.push(ck(format!("\n(Spårningen avbröts av dig efter {} hopp.)", n - 1), 100));
        }
        rs(stream)
    }

    fn win_arp(&mut self, st: &mut State, args: &[String]) -> String {
        let a0 = args.get(0).map(|x| x.to_lowercase()).unwrap_or_default();
        if a0 == "-d" {
            st.dev_mut(&self.dev).arp.clear();
            return String::new();
        }
        if a0 != "-a" && a0 != "-g" {
            return "\nVisar och ändrar översättningstabeller för IP-adresser till fysiska adresser\nsom används av ARP (Address Resolution Protocol).\n\nARP -a".into();
        }
        let up = nic_up(st, &self.dev);
        let h = self.me(st);
        let c = conf(h);
        let Some(c) = c.filter(|_| up) else { return "Inga ARP-poster hittades.".into() };
        let ip = c.ip.clone().unwrap_or_default();
        let mut o = vec![String::new(), format!("Gränssnitt: {} --- 0xc", ip), "  Internetadress        Fysisk adress         Typ".into()];
        let mut keys: Vec<&String> = h.arp.keys().collect();
        keys.sort_by_key(|k| U::ipn(k));
        for k in keys {
            o.push(format!("  {}{}dynamisk", U::pad(k, 22), U::pad(U::mac_dash(&h.arp[k].mac).to_lowercase(), 22)));
        }
        o.push(format!("  {}{}statisk", U::pad(U::broadcast(&ip, &cmask(&c)), 22), U::pad("ff-ff-ff-ff-ff-ff", 22)));
        o.push(format!("  {}{}statisk", U::pad("224.0.0.22", 22), U::pad("01-00-5e-00-00-16", 22)));
        o.push(format!("  {}{}statisk", U::pad("255.255.255.255", 22), U::pad("ff-ff-ff-ff-ff-ff", 22)));
        o.join("\n")
    }

    fn nslookup(&mut self, st: &mut State, args: &[String]) -> String {
        let name = args.get(0).cloned();
        let c = conf(self.me(st));
        let srv = c.as_ref().and_then(|c| c.dns.get(0).cloned());
        let nm = name.clone().unwrap_or_default();
        let Some(srv) = srv else {
            return format!("Standardservern är inte tillgänglig\nServer:  UnKnown\nAddress:  ::1\n\n*** UnKnown hittar inte {}: No response from server", nm);
        };
        let Some(name) = name else {
            return format!("Standardserver:  UnKnown\nAddress:  {}\n\n> (skriv exit för att avsluta – här räcker nslookup <namn>)", srv);
        };
        let rr = S::resolve(st, &self.dev, &name);
        if rr.error.as_deref() == Some("timeout") {
            return format!("DNS request timed out.\n    timeout was 2 seconds.\nServer:  UnKnown\nAddress:  {}\n\n*** Tidsgränsen för begäran till UnKnown överskreds", srv);
        }
        if rr.error.is_some() {
            return format!("Server:  UnKnown\nAddress:  {}\n\n*** UnKnown hittar inte {}: Non-existent domain", srv, name);
        }
        let ip = rr.ip.unwrap_or_default();
        let fq = if !name.contains('.') { format!("{}.nordvik.example", name) } else { name.clone() };
        format!(
            "Server:  UnKnown\nAddress:  {}\n\n{}Namn:    {}\nAddress:  {}",
            srv,
            if U::is_private(&ip) { "" } else { "Icke-auktoritativt svar:\n" },
            fq,
            ip
        )
    }

    fn netsh(&mut self, st: &mut State, line: &str) -> String {
        let id = self.dev.clone();
        // Orden efter "interface ip|ipv4 set address|dns"
        let toks: Vec<&str> = line.split_whitespace().collect();
        let mut kind: Option<(bool, String)> = None; // (är adress, resten)
        for i in 0..toks.len() {
            if toks[i].eq_ignore_ascii_case("interface")
                && toks.get(i + 1).map(|t| t.eq_ignore_ascii_case("ip") || t.eq_ignore_ascii_case("ipv4")).unwrap_or(false)
                && toks.get(i + 2).map(|t| t.eq_ignore_ascii_case("set")).unwrap_or(false)
                && toks.get(i + 3).map(|t| t.eq_ignore_ascii_case("address") || t.eq_ignore_ascii_case("dns")).unwrap_or(false)
            {
                kind = Some((toks[i + 3].eq_ignore_ascii_case("address"), toks[i + 4..].join(" ")));
                break;
            }
        }
        if let Some((is_addr, rest)) = kind {
            // (name=)?"?[^"]*"? namnet, sedan (source=)?(static|dhcp) och resten
            let mut s = rest.as_str();
            if starts_ci(s, "name=") {
                s = &s[5..];
            }
            let after_name: &str = if let Some(q) = s.strip_prefix('"') {
                match q.find('"') {
                    Some(e) => &q[e + 1..],
                    None => "",
                }
            } else {
                match s.find(char::is_whitespace) {
                    Some(e) => &s[e..],
                    None => "",
                }
            };
            let mut t = after_name.trim_start();
            if starts_ci(t, "source=") {
                t = &t[7..];
            }
            let mode = if starts_ci(t, "static") {
                Some(("static", &t[6..]))
            } else if starts_ci(t, "dhcp") {
                Some(("dhcp", &t[4..]))
            } else {
                None
            };
            if let Some((mode, tail)) = mode {
                let tail = tail.trim();
                if is_addr {
                    if mode == "dhcp" {
                        st.dev_mut(&id).nic.as_mut().unwrap().dhcp = true;
                        S::touch(st);
                        S::dhcp(st, &id);
                        return "Ok.".into();
                    }
                    let p: Vec<String> = tail
                        .split_whitespace()
                        .map(|x| match x.find('=') {
                            Some(e) if x[..e].chars().all(|c| c.is_alphanumeric() || c == '_') && e > 0 => x[e + 1..].to_string(),
                            _ => x.to_string(),
                        })
                        .collect();
                    let g = |i: usize| p.get(i).cloned().unwrap_or_default();
                    if !U::is_ip(&g(0)) || !U::is_ip(&g(1)) || !U::is_valid_mask(&g(1)) || (!g(2).is_empty() && !U::is_ip(&g(2))) {
                        return "Den angivna parametern är ogiltig.".into();
                    }
                    let h = st.dev_mut(&id);
                    let dns = match h.nic().static_.as_ref() {
                        Some(sx) => sx.dns.clone(),
                        None => h.lease.as_ref().map(|l| l.dns.clone()).unwrap_or_default(),
                    };
                    let nic = h.nic.as_mut().unwrap();
                    nic.dhcp = false;
                    nic.static_ = Some(crate::net::model::StaticIp { ip: g(0), mask: g(1), gw: if g(2).is_empty() { None } else { Some(g(2)) }, dns });
                    S::release(st, &id);
                    S::touch(st);
                    return "Ok.".into();
                } else {
                    if mode == "static" {
                        let ip = tail.split_whitespace().next().unwrap_or("").to_string();
                        if !U::is_ip(&ip) {
                            return "Den angivna parametern är ogiltig.".into();
                        }
                        let h = st.dev_mut(&id);
                        if let Some(sx) = h.nic.as_mut().unwrap().static_.as_mut() {
                            sx.dns = vec![ip];
                        } else if let Some(l) = h.lease.as_mut() {
                            l.dns = vec![ip];
                        }
                    }
                    S::touch(st);
                    return "Ok.".into();
                }
            }
        }
        let rest = if starts_ci(line.trim_start(), "netsh") { line.trim_start()[5..].trim_start() } else { line };
        format!("Följande kommando hittades inte: {}.", rest)
    }
}
