//! Teknikerns Linux-laptop: grundkommandona (host.js, LinuxShell) inklusive ssh, telnet och screen.
use super::host::*;
use super::{CmdResult, PushReq};
use crate::net::model::*;
use crate::net::sim::{self as S, AclPkt, PingOpts, State};
use crate::net::util as U;

fn man_text(k: &str) -> Option<&'static str> {
    Some(match k {
        "ping" => "PING(8)\n  ping [-c antal] [-s storlek] [-M do] mål\n  -c  antal paket\n  -s  datastorlek i byte (56 som standard, + 28 byte huvud)\n  -M do  sätt DF-biten: routrar får inte dela paketet (hittar MTU-problem)",
        "ssh" => "SSH(1)\n  ssh [-l användare] [användare@]värd\n  Krypterad inloggning. Första gången frågar ssh om värdens nyckel.",
        "curl" => "CURL(1)\n  curl [-I] [-s] URL\n  -I  bara svarshuvudet (statuskod, server)\n  -s  tyst läge\n  Testa en lastbalanserare: for i in 1 2 3 4; do curl -s http://tid/; done",
        "tracepath" => "TRACEPATH(8)\n  tracepath mål\n  Som traceroute, men visar också den minsta MTU:n på vägen (pmtu).",
        "ip" => "IP(8)\n  ip -4 addr show     adresser\n  ip route show       vägar\n  ip route get <ip>   vilken väg ett paket tar\n  ip -4 neigh show    ARP-tabellen",
        "screen" => "SCREEN(1)\n  screen /dev/ttyUSB0 9600\n  Seriell konsol. Ctrl+A K stänger.",
        "nc" => "NC(1)\n  nc -zv värd port\n  Testar om en TCP-port svarar.",
        "grep" => "GREP(1)\n  kommando | grep [-i] [-v] [-c] mönster\n  -i  skiftlägesokänsligt  -v  visa rader som INTE matchar  -c  räkna",
        _ => return None,
    })
}

fn btoa32(s: &str) -> String {
    let alpha: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".chars().collect();
    let mut h = U::hash(s);
    let mut o = String::new();
    for _ in 0..43 {
        o.push(alpha[(h % 64) as usize]);
        h = U::hash(&format!("{}{}", o, s));
    }
    o
}

impl HostSession {
    /// Linux-kommandot med historikhantering (L.handle i host_more.js)
    pub(super) fn lin_top(&mut self, st: &mut State, line: &str) -> CmdResult {
        let before = self.hist.len();
        let rr = self.lin_layer(st, line);
        // Det som skrevs ska stå i historiken, inte kommandot som det översattes till internt
        let t = line.trim();
        if !t.is_empty() && !line.trim_start().starts_with('!') {
            let n = self.hist.len();
            if n == before {
                self.hist.push(t.to_string());
            } else if n > before {
                self.hist.truncate(before + 1);
                self.hist[before] = t.to_string();
            }
        }
        rr
    }

    pub(super) fn lin_base(&mut self, st: &mut State, line: &str) -> CmdResult {
        // !! kör om senaste kommandot (bash skriver ut det först)
        if line.contains("!!") {
            let last = self.hist.iter().rev().find(|x| !x.contains("!!")).cloned();
            let Some(last) = last else { return r("bash: !!: event not found") };
            let nl = line.replace("!!", &last);
            let mut rr = self.lin_top(st, &nl);
            rr.out = format!("{}\n{}", nl, rr.out);
            return rr;
        }
        if let Some(pp) = split_pipe(line) {
            let r0 = self.lin_top(st, &pp.left);
            if let Some(l) = self.hist.last_mut() {
                *l = line.trim().to_string();
            }
            return pipe_filter(r0, &pp);
        }
        let mut a = splitargs(line);
        if a.is_empty() {
            return r("");
        }
        self.hist.push(line.trim().to_string());
        if a[0] == "sudo" {
            a.remove(0);
            if a.is_empty() {
                return r("usage: sudo command");
            }
        }
        let cmd = a[0].clone();
        let id = self.dev.clone();
        match cmd.as_str() {
            "help" => {
                return r(vec![
                    "Kommandon på laptopen:",
                    "  screen /dev/ttyUSB0 9600      konsol mot enheten där konsolkabeln sitter (Ctrl+A K eller Esc stänger)",
                    "  ls /dev/ttyUSB*              se om USB-konsoladaptern syns",
                    "  ssh drift@<ip>               logga in på en enhet över nätet",
                    "  telnet <ip>                  (osäkert – används bara för att testa)",
                    "  ping -c 4 <ip|namn>          ip -4 addr show     ip route show default",
                    "  ip link show                 ip -4 neigh show    resolvectl query <namn>",
                    "  traceroute <ip>              nslookup <namn>     clear",
                    "  curl http://<ip|namn>/       ping -s 1400 -M do <ip>   (stora paket, DF satt)",
                    "  for i in 1 2 3 4; do curl -s http://tid/; done   (testa lastbalanseraren)",
                    "  tracepath <ip>               ip route get <ip>   dig +short <namn>",
                    "  kommando | grep <mönster>    !! (kör om)         man <kommando>",
                    "  date   uptime   whoami   neofetch   fortune   history   echo",
                ]
                .join("\n"));
            }
            "clear" => return CmdResult { clear: true, ..Default::default() },
            "exit" | "logout" => return CmdResult { close: true, ..Default::default() },
            "whoami" => return r("tekniker"),
            "date" => return r(format!("tis 29 sep 2026 {} CEST", clock(st))),
            "uptime" => return r(format!(" {} up 3 days,  2:14,  1 user,  load average: 0,08, 0,12, 0,10", clock(st))),
            "fortune" => {
                let i = (st.random() * FORTUNES.len() as f64).floor() as usize;
                return r(FORTUNES[i.min(FORTUNES.len() - 1)]);
            }
            "neofetch" => {
                let ip = conf(self.me(st)).and_then(|c| c.ip).unwrap_or_else(|| "–".into());
                return r(vec![
                    "        .--.         tekniker@laptop".to_string(),
                    "       |o_o |        ---------------".into(),
                    "       |:_/ |        OS: Ubuntu 24.04 LTS".into(),
                    "      //   \\ \\       Värd: Nordviks teknikerlaptop".into(),
                    "     (|     | )      Skal: bash 5.2".into(),
                    "    /'\\_   _/`\\      Terminal: Krabba-passet".into(),
                    "    \\___)=(___/      Konsol: /dev/ttyUSB0 (ljusblå kabel)".into(),
                    format!("                     IP: {}", ip),
                ]
                .join("\n"));
            }
            "history" => {
                if a.get(1).map(|x| x == "-c").unwrap_or(false) {
                    self.hist.clear();
                    return r("");
                }
                return r(self.hist.iter().enumerate().map(|(i, l)| format!("{}  {}", U::pad_l(i + 1, 5), l)).collect::<Vec<_>>().join("\n"));
            }
            "ifconfig" => return r("Command 'ifconfig' not found, but can be installed with:\nsudo apt install net-tools\n\nTips: använd ip a (adresser) och ip r (vägar) i stället."),
            "arp" => {
                let neigh = self.ip(st, &["neigh".to_string()]);
                let rows: Vec<String> = neigh
                    .split('\n')
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        let p: Vec<&str> = l.split(' ').collect();
                        format!("{}ether   {}   C   enp0s31f6", U::pad(p[0], 22), p.get(4).copied().unwrap_or(""))
                    })
                    .collect();
                return r(if rows.is_empty() { "Address                  HWtype  HWaddress           Flags Mask            Iface".to_string() } else { rows.join("\n") });
            }
            "nc" | "netcat" => return self.nc(st, &a[1..]),
            "curl" | "wget" => {
                let Some(url) = a[1..].iter().find(|x| !x.starts_with('-')).cloned() else {
                    return r(format!("{}: try '{} --help' for more information", cmd, cmd));
                };
                let (t, d) = curl_text(st, &id, &url, a.iter().any(|x| x == "-I"));
                return rd(t, d);
            }
            "for" => return self.for_loop(st, line),
            "hostname" => {
                return r(if a.get(1).map(|x| x == "-I").unwrap_or(false) { conf(self.me(st)).and_then(|c| c.ip).unwrap_or_default() } else { "laptop".to_string() });
            }
            "ls" => return r(self.ls(&a[1..])),
            "dmesg" => return r(self.dmesg()),
            "screen" | "minicom" | "picocom" => return self.screen(&a[1..], &cmd),
            "ip" => return r(self.ip(st, &a[1..])),
            "ping" => return self.lin_ping(st, &a[1..]),
            "traceroute" => return self.trace(st, &a[1..]),
            "tracepath" => return self.tracepath(st, &a[1..]),
            "man" => {
                let k = a.get(1).map(|x| x.to_lowercase()).unwrap_or_default();
                return r(match man_text(&k) {
                    Some(t) => t.to_string(),
                    None if !k.is_empty() => format!("Ingen manualsida för {}. Prova man ping, man ssh, man curl, man tracepath eller man ip.", a[1]),
                    None => "Vilken manualsida vill du ha?\nTill exempel: man ping".to_string(),
                });
            }
            "echo" => {
                let mut t = a[1..].join(" ");
                if t.starts_with('"') || t.starts_with('\'') {
                    t.remove(0);
                }
                if t.ends_with('"') || t.ends_with('\'') {
                    t.pop();
                }
                return r(t.replace("$USER", "tekniker").replace("$HOSTNAME", "laptop"));
            }
            "sl" => {
                return rd(
                    vec![
                        "      ====        ________                ___________",
                        "  _D _|  |_______/        \\__I_I_____===__|_________|",
                        "   |(_)---  |   H\\________/ |   |        =|___ ___|",
                        "   /     |  |   H  |  |     |   |         ||_| |_||",
                        "  |      |  |   H  |__--------------------| [___] |",
                        "  | ________|___H__/__|_____/[][]~\\_______|       |",
                        "  |/ |   |-----------I_____I [][] []  D   |=======|_",
                        "(Du menade nog ls. Tåget går till Borås – via VPN.)",
                    ]
                    .join("\n"),
                    300,
                );
            }
            "resolvectl" => return r(self.resolvectl(st, &a[1..])),
            "nslookup" | "host" | "dig" => {
                let nm = a[1..].iter().find(|x| !x.starts_with('+') && !x.starts_with('-')).cloned();
                if cmd == "dig" && a.iter().any(|x| x == "+short") {
                    let rs_ = S::resolve(st, &id, nm.as_deref().unwrap_or(""));
                    return r(if rs_.error.is_some() { String::new() } else { rs_.ip.unwrap_or_default() });
                }
                return r(self.lookup(st, nm.as_deref()));
            }
            "ssh" => return self.ssh(st, &a[1..]),
            "telnet" => return self.telnet(st, &a[1..]),
            "uname" => return r(if a.get(1).map(|x| x == "-a").unwrap_or(false) { "Linux laptop 6.8.0-45-generic #45-Ubuntu SMP PREEMPT_DYNAMIC x86_64 GNU/Linux" } else { "Linux" }),
            "id" => return r("uid=1000(tekniker) gid=1000(tekniker) grupper=1000(tekniker),4(adm),20(dialout),27(sudo)"),
            "cat" => {
                let f = a.get(1).map(|x| x.as_str()).unwrap_or("");
                if f == "/etc/hosts" {
                    return r("127.0.0.1\tlocalhost\n127.0.1.1\tlaptop\n\n# Nordvik – bra att ha när DNS krånglar\n192.168.1.193\tr-nordvik-1\n192.168.2.193\tr-boras-1\n192.168.1.13\tlb-nordvik tid");
                }
                if f == "/etc/resolv.conf" {
                    return r("# This is /run/systemd/resolve/stub-resolv.conf managed by man:systemd-resolved(8).\nnameserver 127.0.0.53\noptions edns0 trust-ad\nsearch nordvik.example\n\n# Den riktiga DNS-servern: resolvectl status");
                }
                if f == "felrapport.md" {
                    return r("(Felrapporten fyller du i spelet med F.)");
                }
                return r(format!("cat: {}: Filen eller katalogen finns inte", f));
            }
            _ => {}
        }
        r(format!("{}: command not found", cmd))
    }

    fn for_loop(&mut self, st: &mut State, line: &str) -> CmdResult {
        // for i in 1 2 3 4; do curl -s http://tid/; done  – vanligt sätt att testa en lastbalanserare
        let usage = || r("bash: syntax: for i in 1 2 3 4; do curl -s http://<adress>/; done");
        let t = line.trim();
        let parts: Vec<&str> = t.split(';').collect();
        if parts.len() != 3 {
            return usage();
        }
        let toks: Vec<&str> = parts[0].split_whitespace().collect();
        if toks.len() < 4 || toks[0] != "for" || toks[2] != "in" || !toks[1].chars().all(|c| c.is_alphanumeric() || c == '_') {
            return usage();
        }
        let list = &toks[3..];
        let times: usize = if list.len() == 1 && list[0].starts_with("{1..") && list[0].ends_with('}') && is_digits(&list[0][4..list[0].len() - 1]) {
            list[0][4..list[0].len() - 1].parse().unwrap_or(0)
        } else if list.iter().all(|x| is_digits(x)) {
            list.len()
        } else {
            return usage();
        };
        let p1 = parts[1].trim_start();
        let cmdpart = match p1.strip_prefix("do") {
            Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim_start(),
            _ => return usage(),
        };
        if !cmdpart.starts_with("curl") || parts[2].trim() != "done" {
            return usage();
        }
        let ca = splitargs(cmdpart);
        let u2 = ca[1..].iter().find(|x| !x.starts_with('-')).cloned().unwrap_or_default();
        let head_only = ca.iter().any(|x| x == "-I");
        let mut outs = vec![];
        let id = self.dev.clone();
        for _ in 0..times.min(12) {
            let (txt, _) = curl_text(st, &id, &u2, head_only);
            let keep: Vec<&str> = txt
                .split('\n')
                .filter(|l| l.contains("h1") || l.contains("betjänad") || l.contains("curl:") || l.contains("HTTP") || l.contains("Backend"))
                .collect();
            outs.push(keep.join(" "));
        }
        rd(outs.join("\n"), 400)
    }

    fn nc(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let rest: Vec<&String> = args.iter().filter(|x| !x.starts_with('-')).collect();
        let host = rest.get(0).map(|s| s.to_string());
        let port = rest.get(1).and_then(|x| parse_int(x)).filter(|p| *p != 0);
        let (Some(host), Some(port)) = (host, port) else { return r("usage: nc -zv <värd> <port>   (testar om en TCP-port svarar)") };
        let mut ip = host.clone();
        if !U::is_ip(&host) {
            let rr = S::resolve(st, &self.dev, &host);
            if rr.error.is_some() {
                return r(format!("nc: getaddrinfo for host \"{}\" port {}: Name or service not known", host, port));
            }
            ip = rr.ip.unwrap_or_default();
        }
        let p = S::ping(st, &self.dev, &ip, PingOpts::tcp(port));
        if p.ok {
            return rd(format!("Connection to {} {} port [tcp/*] succeeded!", host, port), 300);
        }
        if p.reason.as_deref() == Some("refused") {
            return rd(format!("nc: connect to {} port {} (tcp) failed: Connection refused", host, port), 300);
        }
        rd(format!("nc: connect to {} port {} (tcp) failed: Connection timed out", host, port), 3000)
    }

    pub(super) fn ls(&self, args: &[String]) -> String {
        let p = args.join(" ");
        if p.contains("/dev/tty") {
            if self.console_target.is_none() {
                return format!("ls: cannot access '{}': No such file or directory", p);
            }
            return "/dev/ttyUSB0".into();
        }
        "Dokument  Hämtningar  configs  dagbok.md  felrapport.md".into()
    }

    fn dmesg(&self) -> String {
        if self.console_target.is_none() {
            return "[ 1204.221] usb 1-2: USB disconnect, device number 7".into();
        }
        "[ 1301.117] usb 1-2: new full-speed USB device number 8 using xhci_hcd\n[ 1301.270] usb 1-2: New USB device found, idVendor=0403, idProduct=6001\n[ 1301.271] usb 1-2: Product: FT232R USB UART\n[ 1301.280] ftdi_sio 1-2:1.0: FTDI USB Serial Device converter detected\n[ 1301.282] usb 1-2: FTDI USB Serial Device converter now attached to ttyUSB0".into()
    }

    fn screen(&mut self, args: &[String], cmd: &str) -> CmdResult {
        let mut dev_path = args.iter().find(|x| x.starts_with("/dev/")).cloned();
        let mut speed_arg: Option<String> = args.iter().find(|x| is_digits(x)).cloned();
        if cmd == "picocom" {
            speed_arg = args.iter().position(|x| x == "-b").and_then(|bi| args.get(bi + 1).cloned()).or(Some("9600".into()));
        }
        if cmd == "minicom" {
            speed_arg = Some("9600".into());
            if dev_path.is_none() {
                dev_path = Some("/dev/ttyUSB0".into());
            }
        }
        let Some(dev_path) = dev_path else { return r("Använd: screen /dev/ttyUSB0 9600") };
        let target = self.console_target.clone();
        if dev_path != "/dev/ttyUSB0" || target.is_none() {
            return rd(format!("Cannot exec '{}': No such file or directory", dev_path), 200);
        }
        let speed = parse_int(speed_arg.as_deref().unwrap_or("9600")).unwrap_or(9600) as u32;
        HostSession::push_res(PushReq::Serial { target: target.unwrap(), speed })
    }

    pub(super) fn ip(&mut self, st: &mut State, args: &[String]) -> String {
        let up = nic_up(st, &self.dev);
        let h = self.me(st);
        let c = conf(h);
        let s = args.join(" ");
        let mac = h.mac();
        let words: Vec<&str> = s.split_whitespace().collect();
        let has = |ws: &[&str]| words.iter().any(|w| ws.contains(w));
        // /^(-4\s+)?(link|l)(\s+show)?/
        let first = {
            let mut w = words.iter().peekable();
            if w.peek() == Some(&&"-4") {
                w.next();
            }
            w.next().copied()
        };
        if first.map(|f| f.starts_with('l')).unwrap_or(false) {
            return format!(
                "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN mode DEFAULT group default qlen 1000\n    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00\n2: enp0s31f6: <BROADCAST,MULTICAST{}> mtu 1500 qdisc fq_codel state {} mode DEFAULT group default qlen 1000\n    link/ether {} brd ff:ff:ff:ff:ff:ff",
                if up { ",UP,LOWER_UP" } else { ",UP" },
                if up { "UP" } else { "DOWN" },
                U::mac_colon(&mac)
            );
        }
        if has(&["addr", "a", "address"]) {
            let mut o = format!(
                "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN group default qlen 1000\n    inet 127.0.0.1/8 scope host lo\n       valid_lft forever preferred_lft forever\n2: enp0s31f6: <BROADCAST,MULTICAST{}> mtu 1500 qdisc fq_codel state {} group default qlen 1000",
                if up { ",UP,LOWER_UP" } else { ",UP" },
                if up { "UP" } else { "DOWN" }
            );
            if let Some(c) = &c {
                let ip = c.ip.clone().unwrap_or_default();
                let mask = cmask(c);
                o.push_str(&format!(
                    "\n    inet {}/{} brd {} scope global noprefixroute enp0s31f6\n       valid_lft forever preferred_lft forever",
                    ip,
                    U::mask_to_prefix(&mask).map(|p| p.to_string()).unwrap_or_default(),
                    U::broadcast(&ip, &mask)
                ));
            }
            return o;
        }
        // route get <ip>
        for (i, w) in words.iter().enumerate() {
            if (*w == "route" || *w == "r") && words.get(i + 1) == Some(&"get") && words.get(i + 2).is_some() {
                let Some(c) = &c else { return "RTNETLINK answers: Network is unreachable".into() };
                let dst = words[i + 2];
                if !U::is_ip(dst) {
                    return format!("Error: any valid prefix is expected rather than \"{}\".", dst);
                }
                let ip = c.ip.clone().unwrap_or_default();
                let same = U::same_subnet(dst, &ip, &cmask(c));
                let via = if same { String::new() } else { c.gw.as_ref().map(|g| format!(" via {}", g)).unwrap_or_default() };
                if via.is_empty() && !same {
                    return "RTNETLINK answers: Network is unreachable".into();
                }
                return format!("{}{} dev enp0s31f6 src {} uid 1000 \n    cache ", dst, via, ip);
            }
        }
        if has(&["route", "r"]) {
            let Some(c) = &c else { return String::new() };
            let d = c.gw.as_ref().map(|g| format!("default via {} dev enp0s31f6 proto static metric 100", g)).unwrap_or_default();
            if s.contains("default") {
                return d;
            }
            let ip = c.ip.clone().unwrap_or_default();
            let mask = cmask(c);
            return format!(
                "{}{}/{} dev enp0s31f6 proto kernel scope link src {} metric 100",
                if d.is_empty() { String::new() } else { format!("{}\n", d) },
                U::network(&ip, &mask),
                U::mask_to_prefix(&mask).map(|p| p.to_string()).unwrap_or_default(),
                ip
            );
        }
        if has(&["neigh", "n", "neighbour"]) {
            return h
                .arp
                .iter()
                .map(|(ip, e)| format!("{} dev enp0s31f6 lladdr {} REACHABLE", ip, U::mac_colon(&e.mac)))
                .collect::<Vec<_>>()
                .join("\n");
        }
        "Usage: ip [ OPTIONS ] OBJECT { COMMAND | help }\n       ip -4 addr show | ip link show | ip route show default | ip -4 neigh show".into()
    }

    fn lin_ping(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let id = self.dev.clone();
        let mut n: i64 = 4;
        let mut target: Option<String> = None;
        let mut size: i64 = 56;
        let mut df = false;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "-c" {
                n = pint_or(args.get(i + 1), 4).min(20);
                i += 1;
            } else if args[i] == "-s" {
                size = pint_or(args.get(i + 1), 56).clamp(0, 65507);
                i += 1;
            } else if args[i] == "-M" {
                df = args.get(i + 1).map(|x| x == "do").unwrap_or(false);
                i += 1;
            } else if !args[i].starts_with('-') {
                target = Some(args[i].clone());
            }
            i += 1;
        }
        let Some(target) = target else { return r("ping: usage error: Destination address required") };
        let mut ip = target.clone();
        if !U::is_ip(&target) {
            let rr = S::resolve(st, &id, &target);
            if let Some(e) = &rr.error {
                return rd(format!("ping: {}: Temporary failure in name resolution", target), if e == "timeout" { 2500 } else { 200 });
            }
            ip = rr.ip.unwrap_or_default();
        }
        if df && size > 1472 {
            return r(format!(
                "PING {} ({}) {}({}) bytes of data.\nping: local error: message too long, mtu=1500\n\n--- {} ping statistics ---\n{} packets transmitted, 0 received, +{} errors, 100% packet loss",
                target, ip, size, size + 28, target, n, n
            ));
        }
        let p = S::ping(st, &id, &ip, PingOpts { size: Some(size + 28), df, ..Default::default() });
        let mut stream = vec![ck(format!("PING {} ({}) {}({}) bytes of data.\n", target, ip, size, size + 28), 100)];
        let mut ok = 0i64;
        let mut errs = 0i64;
        let reason = p.reason.clone().unwrap_or_default();
        let me_ip = cip(&conf(self.me(st)));
        for k in 1..=n {
            if !p.ok && reason == "frag" {
                errs += 1;
                stream.push(ck(
                    format!("From {} icmp_seq={} Frag needed and DF set (mtu = {})\n", p.from_ip.clone().unwrap_or_else(|| ip.clone()), k, p.mtu.map(|m| m.to_string()).unwrap_or_default()),
                    600,
                ));
            } else if p.ok && st.random() >= p.loss {
                ok += 1;
                let t = 0.4 + p.hops.len() as f64 * 0.7 + st.random();
                stream.push(ck(format!("{} bytes from {}: icmp_seq={} ttl={} time={:.2} ms\n", size + 8, ip, k, p.ttl, t), 700));
            } else if !p.ok && (reason == "nolink" || reason == "nogw") {
                return r("ping: connect: Network is unreachable");
            } else if !p.ok && reason == "arp" && p.local {
                errs += 1;
                if k % 3 == 0 || k == n {
                    stream.push(ck(format!("From {} icmp_seq={} Destination Host Unreachable\n", me_ip, k), 2800));
                }
            } else if !p.ok && (reason == "net-unreachable" || reason == "acl") && p.from_ip.is_some() {
                errs += 1;
                stream.push(ck(
                    format!("From {} icmp_seq={}{}\n", p.from_ip.clone().unwrap(), k, if reason == "acl" { " Packet filtered" } else { " Destination Net Unreachable" }),
                    700,
                ));
            } else {
                stream.push(ck("", 1000));
            }
        }
        stream.push(ck(
            format!(
                "\n--- {} ping statistics ---\n{} packets transmitted, {} received, {}{}% packet loss, time {}ms",
                target,
                n,
                ok,
                if errs > 0 { format!("+{} errors, ", errs) } else { String::new() },
                jsr((n - ok) as f64 * 100.0 / n as f64),
                n * 1000 - 1000
            ),
            100,
        ));
        rs(stream)
    }

    fn trace(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let Some(target) = args.iter().find(|x| !x.starts_with('-')).cloned() else { return r("Usage: traceroute <host>") };
        let mut ip = target.clone();
        if !U::is_ip(&target) {
            let rr = S::resolve(st, &self.dev, &target);
            if rr.error.is_some() {
                return r(format!("{}: Temporary failure in name resolution", target));
            }
            ip = rr.ip.unwrap_or_default();
        }
        let (fwd, full) = S::traceroute(st, &self.dev, &ip, None);
        let mut stream = vec![ck(format!("traceroute to {} ({}), 30 hops max, 60 byte packets\n", target, ip), 100)];
        let mut n = 1;
        for hp in &fwd.hops {
            stream.push(ck(
                format!("{}  {} ({})  0.{} ms  0.{} ms  0.{} ms\n", U::pad_l(n, 2), hp.ip, hp.ip, 300 + n * 91, 280 + n * 77, 310 + n * 55),
                400,
            ));
            n += 1;
        }
        if full.ok {
            if fwd.hops.last().map(|h| h.ip != ip).unwrap_or(true) {
                stream.push(ck(format!("{}  {} ({})  1.012 ms  0.981 ms  0.977 ms\n", U::pad_l(n, 2), ip, ip), 400));
            }
        } else {
            for _ in 0..4 {
                stream.push(ck(format!("{}  * * *\n", U::pad_l(n, 2)), 1500));
                n += 1;
            }
        }
        rs(stream)
    }

    // tracepath: vägen och den minsta MTU:n längs vägen (PMTU)
    fn tracepath(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let Some(target) = args.iter().find(|x| !x.starts_with('-')).cloned() else { return r("Usage: tracepath [-n] <destination>") };
        let mut ip = target.clone();
        if !U::is_ip(&target) {
            let rr = S::resolve(st, &self.dev, &target);
            if rr.error.is_some() {
                return r(format!("tracepath: {}: Name or service not known", target));
            }
            ip = rr.ip.unwrap_or_default();
        }
        let (fwd, full) = S::traceroute(st, &self.dev, &ip, None);
        let big = S::ping(st, &self.dev, &ip, PingOpts { size: Some(1500), df: true, ..Default::default() });
        let pmtu: i64 = if !big.ok && big.reason.as_deref() == Some("frag") { big.mtu.unwrap_or(1500) } else { 1500 };
        let mut stream = vec![ck(" 1?: [LOCALHOST]                      pmtu 1500\n", 150)];
        let mut n = 1;
        for (i, hp) in fwd.hops.iter().enumerate() {
            let extra = if i == 0 && pmtu < 1500 {
                format!("\n{}:  {}{:.3}ms pmtu {}", U::pad_l(n, 2), U::pad(&hp.ip, 34), 0.5 + n as f64 * 0.6, pmtu)
            } else {
                String::new()
            };
            stream.push(ck(format!("{}:  {}{:.3}ms {}\n", U::pad_l(n, 2), U::pad(&hp.ip, 34), 0.4 + n as f64 * 0.6, extra), 350));
            n += 1;
        }
        if full.ok {
            if fwd.hops.last().map(|h| h.ip != ip).unwrap_or(true) {
                stream.push(ck(format!("{}:  {}{:.3}ms reached\n", U::pad_l(n, 2), U::pad(&ip, 34), 1.0 + n as f64 * 0.6), 350));
                n += 1;
            }
            stream.push(ck(format!("     Resume: pmtu {} hops {} back {}", pmtu, n - 1, n - 1), 100));
        } else {
            for _ in 0..3 {
                stream.push(ck(format!("{}:  no reply\n", U::pad_l(n, 2)), 1200));
                n += 1;
            }
            stream.push(ck(format!("     Too many hops: pmtu {}", pmtu), 100));
        }
        rs(stream)
    }

    fn resolvectl(&mut self, st: &mut State, args: &[String]) -> String {
        let c = conf(self.me(st));
        if args.get(0).map(|x| x == "query").unwrap_or(false) && args.get(1).is_some() {
            let name = args[1].clone();
            let rr = S::resolve(st, &self.dev, &name);
            if let Some(e) = &rr.error {
                return format!(
                    "{}: resolve call failed: {}",
                    name,
                    if e == "timeout" { "All attempts to contact name servers or networks failed".to_string() } else { format!("'{}' not found", name) }
                );
            }
            return format!(
                "{}: {}                          -- link: enp0s31f6\n\n-- Information acquired via protocol DNS in 3.1ms.\n-- Data is authenticated: no; Data was acquired via local or encrypted transport: no",
                name,
                rr.ip.unwrap_or_default()
            );
        }
        let dns0 = c.as_ref().and_then(|c| c.dns.get(0).cloned()).unwrap_or_default();
        let dnsall = c.as_ref().map(|c| c.dns.join(" ")).unwrap_or_default();
        format!("Global\n       Protocols: +LLMNR +mDNS -DNSOverTLS DNSSEC=no/unsupported\nresolv.conf mode: stub\n\nLink 2 (enp0s31f6)\n    Current Scopes: DNS\n         Protocols: +DefaultRoute -LLMNR -mDNS -DNSOverTLS DNSSEC=no/unsupported\nCurrent DNS Server: {}\n       DNS Servers: {}\n        DNS Domain: nordvik.example", dns0, dnsall)
    }

    fn lookup(&mut self, st: &mut State, name: Option<&str>) -> String {
        let Some(name) = name else { return "Usage: nslookup <namn>".into() };
        let c = conf(self.me(st));
        let dns0 = c.as_ref().and_then(|c| c.dns.get(0).cloned()).unwrap_or_default();
        let rr = S::resolve(st, &self.dev, name);
        if rr.error.as_deref() == Some("timeout") {
            return format!(";; communications error to {}#53: timed out\n;; no servers could be reached", dns0);
        }
        if rr.error.is_some() {
            return format!("Server:\t\t{0}\nAddress:\t{0}#53\n\n** server can't find {1}: NXDOMAIN", dns0, name);
        }
        format!(
            "Server:\t\t{0}\nAddress:\t{0}#53\n\nName:\t{1}\nAddress: {2}",
            dns0,
            if !name.contains('.') { format!("{}.nordvik.example", name) } else { name.to_string() },
            rr.ip.unwrap_or_default()
        )
    }

    fn find_ios_by_ip(st: &mut State, ip: &str) -> Option<String> {
        let d = S::get(st);
        let mut hit = None;
        for e in d.eps.values() {
            if e.ip.as_deref() == Some(ip) && e.up && st.devices.get(&e.dev).map(|x| x.os == Os::Ios).unwrap_or(false) {
                hit = Some(e.dev.clone());
            }
        }
        hit
    }

    pub(super) fn ssh(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let id = self.dev.clone();
        let mut user: Option<String> = None;
        let mut host: Option<String> = None;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "-l" {
                user = args.get(i + 1).cloned();
                i += 1;
            } else if !args[i].starts_with('-') {
                host = Some(args[i].clone());
            }
            i += 1;
        }
        let Some(mut host) = host else { return r("usage: ssh [-l login_name] [user@]hostname") };
        if let Some(at) = host.find('@') {
            if at > 0 {
                let parts: Vec<String> = host.split('@').map(|s| s.to_string()).collect();
                user = Some(parts[0].clone());
                host = parts.get(1).cloned().unwrap_or_default();
            }
        }
        let user = user.unwrap_or_else(|| "tekniker".into());
        let mut ip = host.clone();
        if !U::is_ip(&host) {
            let rr = S::resolve(st, &id, &host);
            if rr.error.is_some() {
                return r(format!("ssh: Could not resolve hostname {}: Name or service not known", host));
            }
            ip = rr.ip.unwrap_or_default();
        }
        let p = S::ping(st, &id, &ip, PingOpts::tcp(22));
        if !p.ok {
            let rsn = p.reason.clone().unwrap_or_default();
            if rsn == "refused" {
                return rd(format!("ssh: connect to host {} port 22: Connection refused", host), 300);
            }
            if rsn == "nolink" || rsn == "nogw" {
                return r(format!("ssh: connect to host {} port 22: Network is unreachable", host));
            }
            return rd(format!("ssh: connect to host {} port 22: Connection timed out", host), 3500);
        }
        let dev_id = Self::find_ios_by_ip(st, &ip).or(p.at.clone()).unwrap_or_default();
        let ok_dev = st.devices.get(&dev_id).map(|d| d.os == Os::Ios || d.kind == Kind::Wlc || d.kind == Kind::Lb).unwrap_or(false);
        if !ok_dev {
            return r(format!("ssh: connect to host {} port 22: Connection refused", host));
        }
        let (kind, ac) = {
            let d = st.dev(&dev_id);
            (d.kind, d.config.as_ref().and_then(|c| c.lines.vty0_4.access_class.clone()))
        };
        // access-class på vty-linjerna: källadressen måste släppas igenom av listan
        if let (Some(ac), Some(src)) = (ac, p.src_used.clone()) {
            if !S::acl_eval(st, &dev_id, &ac, &AclPkt { src, dst: ip.clone(), proto: "tcp".into(), dport: Some(22) }) {
                return rd(format!("ssh: connect to host {} port 22: Connection refused", host), 300);
            }
        }
        if kind == Kind::Lb {
            self.set_pending(format!("{}@{}'s password: ", user, host), true, PendKind::LbPass { dev: dev_id });
            return r("");
        }
        if !self.known.iter().any(|k| *k == ip) {
            let d = st.dev(&dev_id);
            let base = if d.base_mac.is_empty() { d.mac() } else { d.base_mac.clone() };
            let fp = btoa32(&format!("{}{}", base, d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| "wlc".into())));
            self.set_pending(
                format!(
                    "The authenticity of host '{} ({})' can't be established.\nRSA key fingerprint is SHA256:{}.\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ",
                    host, ip, fp
                ),
                false,
                PendKind::HostKey { host, ip, dev: dev_id, user },
            );
            return r("");
        }
        self.ssh_go(st, &host, &dev_id, &user)
    }

    fn ssh_go(&mut self, st: &mut State, host: &str, dev_id: &str, user: &str) -> CmdResult {
        if st.dev(dev_id).kind == Kind::Wlc {
            return HostSession::push_res(PushReq::Wlc { dev: dev_id.to_string() });
        }
        self.ssh_ask(host, dev_id, user);
        // Första gången visas lösenordet som spelet har satt, så att du vet vad du ska skriva
        let gu = st.dev(dev_id).config.as_ref().and_then(|c| c.users.get(user).cloned());
        if let Some(gu) = gu {
            if gu.by_game && !self.pw_shown {
                self.pw_shown = true;
                return r(format!("💡 Kontot {} har lösenordet {} (står på lappen vid laptopen).", user, gu.plain));
            }
        }
        r("")
    }

    fn ssh_ask(&mut self, host: &str, dev_id: &str, user: &str) {
        self.set_pending(
            format!("{}@{}'s password: ", user, host),
            true,
            PendKind::SshPass { host: host.to_string(), dev: dev_id.to_string(), user: user.to_string() },
        );
    }

    pub(super) fn telnet(&mut self, st: &mut State, args: &[String]) -> CmdResult {
        let Some(host) = args.get(0).cloned() else { return r("usage: telnet [host [port]]") };
        let ip = host.clone();
        let p = S::ping(st, &self.dev, &ip, PingOpts::tcp(23));
        let head = format!("Trying {}...\n", ip);
        if !p.ok {
            if p.reason.as_deref() == Some("refused") {
                return rd(format!("{}telnet: Unable to connect to remote host: Connection refused", head), 300);
            }
            return rd(format!("{}telnet: Unable to connect to remote host: Connection timed out", head), 3500);
        }
        let at = p.at.clone().unwrap_or_default();
        let line = match st.devices.get(&at).and_then(|d| d.config.as_ref()) {
            Some(c) => {
                if c.lines.vty0_4.transport == "ssh" { c.lines.vty5_15.clone() } else { c.lines.vty0_4.clone() }
            }
            None => return rd(format!("{}telnet: Unable to connect to remote host: Connection refused", head), 300),
        };
        if !line.login_local && line.password.as_deref().map(|p| p.is_empty()).unwrap_or(true) {
            return r(format!("{}Connected to {}.\nEscape character is '^]'.\n\n\nPassword required, but none set\nConnection closed by foreign host.", head, ip));
        }
        self.events.push(format!("telnet:{}", at));
        self.set_pending("Username: ", false, PendKind::TelUser { dev: at });
        r(format!("{}Connected to {}.\nEscape character is '^]'.\n\n\nUser Access Verification\n", head, ip))
    }

    /// Svar på en väntande fråga
    pub(super) fn answer(&mut self, st: &mut State, p: Pending, line: &str) -> CmdResult {
        match p.kind {
            PendKind::LbPass { dev } => HostSession::push_res(PushReq::Lb { dev }),
            PendKind::HostKey { host, ip, dev, user } => {
                if line.trim() != "yes" {
                    return r("Host key verification failed.");
                }
                self.known.push(ip);
                let mut rr = self.ssh_go(st, &host, &dev, &user);
                rr.out = format!("Warning: Permanently added '{}' (RSA) to the list of known hosts.{}", host, if rr.out.is_empty() { String::new() } else { format!("\n{}", rr.out) });
                rr
            }
            PendKind::SshPass { host, dev, user } => {
                let (ok_login, tip, privileged) = {
                    let c = st.dev(&dev).cfg();
                    let ln = &c.lines.vty0_4;
                    let u = c.users.get(&user);
                    let ok = if ln.login_local { u.map(|u| u.plain == line).unwrap_or(false) } else { ln.password.as_deref().map(|x| !x.is_empty() && x == line).unwrap_or(false) };
                    let privd = u.map(|u| u.priv_ == 15).unwrap_or(false) || ln.priv_level == Some(15);
                    (ok, pw_tip(c, &user, ln), privd)
                };
                if !ok_login {
                    self.tries += 1;
                    if self.tries >= 3 {
                        self.tries = 0;
                        return r(format!("{}@{}: Permission denied (publickey,keyboard-interactive,password).{}", user, host, tip));
                    }
                    self.ssh_ask(&host, &dev, &user);
                    return r(format!("Permission denied, please try again.{}", tip));
                }
                self.tries = 0;
                HostSession::push_res(PushReq::Ios { dev, via: "ssh".into(), privileged, user: Some(user) })
            }
            PendKind::TelUser { dev } => {
                self.set_pending("Password: ", true, PendKind::TelPass { dev, user: line.to_string() });
                r("")
            }
            PendKind::TelPass { dev, user } => {
                let c = st.dev(&dev).cfg();
                let ln = if c.lines.vty0_4.transport == "ssh" { &c.lines.vty5_15 } else { &c.lines.vty0_4 };
                let uu = c.users.get(&user);
                let ok = if ln.login_local { uu.map(|u| u.plain == line).unwrap_or(false) } else { ln.password.as_deref() == Some(line) };
                if !ok {
                    return r(format!("% Login invalid\n\nConnection closed by foreign host.{}", pw_tip(c, &user, ln)));
                }
                let privileged = uu.map(|u| u.priv_ == 15).unwrap_or(false) || ln.priv_level == Some(15);
                HostSession::push_res(PushReq::Ios { dev, via: "telnet".into(), privileged, user: Some(user) })
            }
            PendKind::ScpPass { host, dev, user, path, dst } => self.scp_answer(st, &host, &dev, &user, &path, &dst, line),
        }
    }
}
