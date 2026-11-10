//! Cisco IOS-kommandotolk: lägen, förkortningar, ?, tab och felmeddelanden.
//! Ramverket (trie, parser, session) från början av js/cli/ios.js. Själva kommandona
//! registreras i ios_cmds.rs, ios_more.rs och ios_dyn.rs med `Reg::def`.
use super::{CmdResult, Cli};
use crate::net::sim::{self as S, State};
use crate::net::util::{self as U, VlanList};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ------------------------------------------------------------------ Värden och argument

#[derive(Clone, Debug)]
pub enum Val {
    /// <word>, <iphost>, <ip>, <mask>, <wild>, <if> (normaliserat interfacenamn)
    Str(String),
    /// <n> eller <n:a-b>
    Num(i64),
    /// <vlist>
    Vlist(VlanList),
    /// <text>: resten av raden från och med detta ord
    Text(String),
}

impl Val {
    pub fn s(&self) -> &str {
        match self {
            Val::Str(s) | Val::Text(s) => s,
            _ => "",
        }
    }
    pub fn n(&self) -> i64 {
        match self {
            Val::Num(n) => *n,
            Val::Str(s) | Val::Text(s) => s.parse().unwrap_or(0),
            _ => 0,
        }
    }
    pub fn vl(&self) -> VlanList {
        match self {
            Val::Vlist(v) => v.clone(),
            _ => VlanList::List(vec![]),
        }
    }
    /// VLAN-listan som vektor (All blir tom)
    pub fn vlans(&self) -> Vec<u16> {
        match self {
            Val::Vlist(VlanList::List(v)) => v.clone(),
            _ => vec![],
        }
    }
    pub fn is_all(&self) -> bool {
        matches!(self, Val::Vlist(VlanList::All))
    }
}

/// Argument till ett kommando: v = parametervärden i ordning, neg = "no"-formen,
/// kws = nyckelorden som matchade (fullständiga), line = hela raden som den skrevs.
pub struct Args {
    pub v: Vec<Val>,
    pub neg: bool,
    pub kws: Vec<String>,
    pub line: String,
}

impl Args {
    pub fn s(&self, i: usize) -> &str {
        self.v.get(i).map(|v| v.s()).unwrap_or("")
    }
    pub fn n(&self, i: usize) -> i64 {
        self.v.get(i).map(|v| v.n()).unwrap_or(0)
    }
    pub fn has_kw(&self, k: &str) -> bool {
        self.kws.iter().any(|x| x == k)
    }
}

pub type CmdFn = Rc<dyn Fn(&mut IosSession, &mut State, &Args) -> CmdResult>;
pub type PendingFn = Box<dyn FnOnce(&mut IosSession, &mut State, &str) -> CmdResult>;

/// En fråga som väntar på svar (t.ex. "Destination filename [startup-config]? ")
pub struct Pending {
    pub prompt: String,
    pub secret: bool,
    pub f: PendingFn,
}

// ------------------------------------------------------------------ Trie

#[derive(Clone)]
pub struct CmdDef {
    pub f: CmdFn,
    /// "no"-formen är tillåten
    pub no: bool,
    /// Bara "no"-formen är tillåten
    pub no_only: bool,
    pub pattern: String,
}

#[derive(Default, Clone)]
pub struct Node {
    /// Nyckelord i den ordning de lades till
    pub kw: Vec<(String, Node)>,
    pub params: Vec<(String, Node)>,
    pub cmd: Option<CmdDef>,
}

impl Node {
    pub fn kw_get(&self, k: &str) -> Option<&Node> {
        self.kw.iter().find(|(x, _)| x == k).map(|(_, n)| n)
    }
    pub fn kw_names(&self) -> Vec<String> {
        self.kw.iter().map(|(k, _)| k.clone()).collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DefOpts {
    pub no: bool,
    pub no_only: bool,
}
impl Default for DefOpts {
    fn default() -> Self {
        DefOpts { no: true, no_only: false }
    }
}
pub const NO_NEG: DefOpts = DefOpts { no: false, no_only: false };
pub const NO_ONLY: DefOpts = DefOpts { no: true, no_only: true };

/// Lägena: "user", "exec", "config", "if", "vlan", "line", "dhcp", "acl", "isakmp", "tset", "cmap", "ospf", "bgp"
#[derive(Default)]
pub struct Reg {
    pub tries: HashMap<String, Node>,
}

impl Reg {
    /// Registrera ett kommando i ett eller flera lägen (mellanslagsseparerade), som def() i ios.js.
    /// Mönstret är ord och parametrar: <ip> <mask> <wild> <word> <iphost> <text> <if> <vlist> <n> <n:1-4094>
    pub fn def<F>(&mut self, modes: &str, pattern: &str, f: F, opts: DefOpts)
    where
        F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
    {
        let f: CmdFn = Rc::new(f);
        for m in modes.split(' ') {
            let mut node = self.tries.entry(m.to_string()).or_default();
            for t in pattern.split(' ') {
                if t.starts_with('<') {
                    let idx = match node.params.iter().position(|(s, _)| s == t) {
                        Some(i) => i,
                        None => {
                            node.params.push((t.to_string(), Node::default()));
                            node.params.len() - 1
                        }
                    };
                    node = &mut node.params[idx].1;
                } else {
                    let idx = match node.kw.iter().position(|(s, _)| s == t) {
                        Some(i) => i,
                        None => {
                            node.kw.push((t.to_string(), Node::default()));
                            node.kw.len() - 1
                        }
                    };
                    node = &mut node.kw[idx].1;
                }
            }
            node.cmd = Some(CmdDef { f: f.clone(), no: opts.no, no_only: opts.no_only, pattern: pattern.to_string() });
        }
    }
    /// Kortform: def med standardinställningar (no tillåtet)
    pub fn d<F>(&mut self, modes: &str, pattern: &str, f: F)
    where
        F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
    {
        self.def(modes, pattern, f, DefOpts::default());
    }
    /// show-kommando: i exec (och user om user_too), utan no-form
    pub fn show<F>(&mut self, pattern: &str, f: F, user_too: bool)
    where
        F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
    {
        self.def(if user_too { "user exec" } else { "exec" }, &format!("show {}", pattern), f, NO_NEG);
    }
}

thread_local! {
    static TRIES: RefCell<Option<Rc<HashMap<String, Node>>>> = const { RefCell::new(None) };
}

/// Alla lägens trie, byggs första gången.
pub fn tries() -> Rc<HashMap<String, Node>> {
    TRIES.with(|t| {
        if let Some(x) = t.borrow().as_ref() {
            return x.clone();
        }
        let mut reg = Reg::default();
        super::ios_cmds::register(&mut reg);
        super::ios_more::register(&mut reg);
        super::ios_dyn::register(&mut reg);
        let rc = Rc::new(reg.tries);
        *t.borrow_mut() = Some(rc.clone());
        rc
    })
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub t: String,
    pub pos: usize,
}

pub fn tokenize(line: &str) -> Vec<Tok> {
    let mut toks = vec![];
    let mut start: Option<usize> = None;
    for (i, c) in line.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                toks.push(Tok { t: line[s..i].to_string(), pos: s });
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        toks.push(Tok { t: line[s..].to_string(), pos: s });
    }
    toks
}

struct Accept {
    n: usize,
    v: Option<Val>,
    rest: bool,
}

fn param_accept(spec: &str, toks: &[Tok], i: usize) -> Option<Accept> {
    let t = &toks[i].t;
    let inner = &spec[1..spec.len() - 1];
    let (ty, rng) = match inner.split_once(':') {
        Some((a, b)) => {
            let mut p = b.split('-').map(|x| x.parse::<i64>().unwrap_or(0));
            (a, Some((p.next().unwrap_or(0), p.next().unwrap_or(0))))
        }
        None => (inner, None),
    };
    match ty {
        "ip" | "mask" | "wild" => U::is_ip(t).then(|| Accept { n: 1, v: Some(Val::Str(t.clone())), rest: false }),
        "n" => {
            if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let v: i64 = t.parse().ok()?;
            if let Some((a, b)) = rng {
                if v < a || v > b {
                    return None;
                }
            }
            Some(Accept { n: 1, v: Some(Val::Num(v)), rest: false })
        }
        "word" | "iphost" => Some(Accept { n: 1, v: Some(Val::Str(t.clone())), rest: false }),
        "vlist" => U::parse_vlan_list(t).map(|l| Accept { n: 1, v: Some(Val::Vlist(l)), rest: false }),
        "text" => Some(Accept { n: toks.len() - i, v: None, rest: true }),
        "if" => {
            if let Some(n1) = U::norm_if(t) {
                return Some(Accept { n: 1, v: Some(Val::Str(n1)), rest: false });
            }
            if i + 1 < toks.len() && t.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
                if let Some(n2) = U::norm_if(&format!("{}{}", t, toks[i + 1].t)) {
                    return Some(Accept { n: 2, v: Some(Val::Str(n2)), rest: false });
                }
            }
            None
        }
        _ => None,
    }
}

pub enum ParseRes<'a> {
    Ok { cmd: CmdDef, vals: Vec<Val>, kws: Vec<String>, node: &'a Node },
    Ambig { pos: usize },
    Invalid { pos: usize },
    Incomplete { node: &'a Node },
}

pub fn parse<'a>(root: &'a Node, toks: &[Tok], line: &str) -> ParseRes<'a> {
    let mut node = root;
    let mut vals = vec![];
    let mut kws = vec![];
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].t.to_lowercase();
        let matches: Vec<&(String, Node)> = if let Some(e) = node.kw.iter().find(|(k, _)| *k == t) {
            vec![e]
        } else {
            node.kw.iter().filter(|(k, _)| k.starts_with(&t)).collect()
        };
        if matches.len() == 1 {
            kws.push(matches[0].0.clone());
            node = &matches[0].1;
            i += 1;
            continue;
        }
        if matches.len() > 1 {
            return ParseRes::Ambig { pos: i };
        }
        let mut took = false;
        for (spec, pnode) in &node.params {
            if let Some(acc) = param_accept(spec, toks, i) {
                if acc.rest {
                    vals.push(Val::Text(line[toks[i].pos..].trim().to_string()));
                } else if let Some(v) = acc.v {
                    vals.push(v);
                }
                node = pnode;
                i += acc.n;
                took = true;
                break;
            }
        }
        if !took {
            return ParseRes::Invalid { pos: i };
        }
    }
    match &node.cmd {
        Some(c) => ParseRes::Ok { cmd: c.clone(), vals, kws, node },
        None => ParseRes::Incomplete { node },
    }
}

// ------------------------------------------------------------------ Session

/// Kontext för undernivåerna i konfigurationsläget.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub ifs: Vec<String>,
    pub range: bool,
    pub sub: bool,
    pub vlans: Vec<u16>,
    /// "con", "vty0_4", "vty5_15"
    pub lines: Vec<String>,
    pub pool: Option<String>,
    pub acl: Option<String>,
    pub acl_extended: bool,
    pub policy: Option<i64>,
    pub ts: Option<String>,
    pub map: Option<String>,
    pub seq: Option<i64>,
}

pub struct IosSession {
    pub dev: String,
    /// "console", "ssh" eller "telnet"
    pub via: String,
    /// "user", "exec", "config", "if", "vlan", "line", "dhcp", "acl", "isakmp", "tset", "cmap", "ospf", "bgp"
    pub mode: String,
    pub ctx: Ctx,
    pub pending: Option<Pending>,
    pub closed: bool,
    pub history: Vec<String>,
    pub user: Option<String>,
    pub booting: bool,
    pub monitor: bool,
    pub in_alias: bool,
}

pub fn result(out: impl Into<String>) -> CmdResult {
    CmdResult::out(out)
}

impl IosSession {
    pub fn new(st: &State, dev: &str, via: &str, privileged: bool, user: Option<String>) -> IosSession {
        let _ = st;
        IosSession {
            dev: dev.to_string(),
            via: via.to_string(),
            mode: if privileged { "exec" } else { "user" }.into(),
            ctx: Ctx::default(),
            pending: None,
            closed: false,
            history: vec![],
            user,
            booting: false,
            monitor: false,
            in_alias: false,
        }
    }
    pub fn console(st: &State, dev: &str) -> IosSession {
        IosSession::new(st, dev, "console", false, None)
    }

    pub fn prompt(&self, st: &State) -> String {
        let h = &st.dev(&self.dev).cfg().hostname;
        let suf = match self.mode.as_str() {
            "user" => ">",
            "exec" => "#",
            "config" => "(config)#",
            "if" => {
                if self.ctx.range {
                    "(config-if-range)#"
                } else if self.ctx.sub {
                    "(config-subif)#"
                } else {
                    "(config-if)#"
                }
            }
            "vlan" => "(config-vlan)#",
            "line" => "(config-line)#",
            "dhcp" => "(dhcp-config)#",
            "acl" => {
                if self.ctx.acl_extended {
                    "(config-ext-nacl)#"
                } else {
                    "(config-std-nacl)#"
                }
            }
            "isakmp" => "(config-isakmp)#",
            "tset" => "(cfg-crypto-trans)#",
            "cmap" => "(config-crypto-map)#",
            "ospf" | "bgp" => "(config-router)#",
            _ => "#",
        };
        format!("{}{}", h, suf)
    }

    /// Ställ en fråga; svaret går till f.
    pub fn ask<F>(&mut self, prompt: impl Into<String>, secret: bool, f: F)
    where
        F: FnOnce(&mut IosSession, &mut State, &str) -> CmdResult + 'static,
    {
        self.pending = Some(Pending { prompt: prompt.into(), secret, f: Box::new(f) });
    }

    pub fn is_router(&self, st: &State) -> bool {
        st.dev(&self.dev).kind == crate::net::model::Kind::Router
    }
    pub fn is_switch(&self, st: &State) -> bool {
        st.dev(&self.dev).kind == crate::net::model::Kind::Switch
    }

    pub fn handle_line(&mut self, st: &mut State, raw: &str) -> CmdResult {
        let line = raw.trim_end().to_string();
        if let Some(p) = self.pending.take() {
            return (p.f)(self, st, line.trim());
        }
        if line.trim().is_empty() || line.trim().starts_with('!') {
            return CmdResult::empty();
        }
        self.history.push(line.clone());
        let mut line2 = line.clone();
        let mut pipe: Option<(String, String)> = None;
        // ^(.*?)\s+\|\s+(\S+)\s*(.*)$
        if let Some(idx) = find_pipe(&line) {
            let before = line[..idx].trim_end().to_string();
            let after = line[idx + 1..].trim_start();
            let (op, arg) = match after.split_once(char::is_whitespace) {
                Some((a, b)) => (a.to_lowercase(), b.trim_start().to_string()),
                None => (after.to_lowercase(), String::new()),
            };
            if !op.is_empty() {
                line2 = before;
                pipe = Some((op, arg));
            }
        }
        let mut r = self.exec(st, &line2);
        if let Some((op, arg)) = &pipe {
            if !r.out.is_empty() {
                r.out = apply_pipe(&r.out, op, arg);
            }
        }
        S::refresh(st);
        // Broadcaststorm: konsolen svarar trögt
        let d = S::get(st);
        if d.storm.values().any(|v| v.iter().any(|x| *x == self.dev)) {
            r.delay += 1800;
        }
        r
    }

    pub fn err_at(&self, st: &State, line: &str, pos: usize, toks: &[Tok]) -> String {
        let off = self.prompt(st).len() + toks.get(pos).map(|t| t.pos).unwrap_or(line.len());
        format!("{}^\n% Invalid input detected at '^' marker.\n", " ".repeat(off))
    }

    /// Markör under sista förekomsten av ordet (invalidMarker i ios.js)
    pub fn invalid_marker(&self, st: &State, line: &str, word: &str) -> String {
        let idx = line.to_lowercase().rfind(&word.to_lowercase()).unwrap_or(0);
        format!("{}^\n% Invalid input detected at '^' marker.\n", " ".repeat(self.prompt(st).len() + idx))
    }

    pub fn leave_sub(&mut self) {
        self.ctx = Ctx::default();
    }

    pub fn exec(&mut self, st: &mut State, line: &str) -> CmdResult {
        let toks = tokenize(line);
        // alias exec: ett eget kortkommando byts mot det riktiga kommandot
        if (self.mode == "exec" || self.mode == "user") && !toks.is_empty() && !self.in_alias {
            let al = st.dev(&self.dev).cfg().aliases.get(&toks[0].t).cloned();
            if let Some(a) = al {
                self.in_alias = true;
                let rest = &line[toks[0].pos + toks[0].t.len()..];
                let r = self.exec(st, &format!("{}{}", a, rest));
                self.in_alias = false;
                return r;
            }
        }
        let mode = self.mode.clone();
        let tr = tries();
        let in_sub = mode != "user" && mode != "exec";
        // "do" i konfigurationslägen
        if in_sub && !toks.is_empty() && toks[0].t.to_lowercase() == "do" {
            let saved = self.mode.clone();
            self.mode = "exec".into();
            let sub = line.trim_start()[2..].trim_start().to_string();
            let r = self.exec(st, &sub);
            if self.mode == "exec" {
                self.mode = saved;
            }
            return r;
        }
        let mut neg = false;
        let mut start = 0;
        if in_sub && !toks.is_empty() && toks[0].t.to_lowercase() == "no" {
            neg = true;
            start = 1;
        }
        let ptoks: Vec<Tok> = toks[start..].to_vec();
        let empty = Node::default();
        let root = tr.get(&mode).unwrap_or(&empty);
        let mut pr = parse(root, &ptoks, line);
        // Undernivå: prova globala konfigurationen
        if !matches!(pr, ParseRes::Ok { .. }) && mode != "config" && in_sub {
            if let Some(cfg_root) = tr.get("config") {
                let gp = parse(cfg_root, &ptoks, line);
                if let ParseRes::Ok { .. } = gp {
                    self.leave_sub();
                    self.mode = "config".into();
                    pr = gp;
                }
            }
        }
        match pr {
            ParseRes::Ambig { .. } => result(format!("% Ambiguous command:  \"{}\"", line.trim())),
            ParseRes::Incomplete { .. } => result("% Incomplete command.\n"),
            ParseRes::Invalid { pos } => {
                let kw: Vec<String> = root.kw_names().into_iter().filter(|k| k.len() > 2).collect();
                let kwr: Vec<&str> = kw.iter().map(|s| s.as_str()).collect();
                if (mode == "user" || mode == "exec") && pos == 0 {
                    let w0 = ptoks[0].t.to_lowercase();
                    let known = tr.get("exec").map(|n| n.kw.iter().any(|(k, _)| k.starts_with(&w0))).unwrap_or(false);
                    if known {
                        return result(self.err_at(st, line, 0, &toks));
                    }
                    if let Some(sg) = super::suggest_cmd(&w0, &kwr) {
                        return result(format!("{}💡 Menade du {}?", self.err_at(st, line, 0, &toks), sg));
                    }
                    let c = st.dev(&self.dev).cfg();
                    let ns = c.name_servers.first().cloned().unwrap_or_else(|| "255.255.255.255".into());
                    if c.domain_lookup {
                        return CmdResult::delayed(
                            format!(
                                "Translating \"{}\"...domain server ({})\n% Unknown command or computer name, or unable to find computer address",
                                ptoks[0].t, ns
                            ),
                            2500,
                        );
                    }
                    return result("% Unknown command or computer name, or unable to find computer address");
                }
                let bad = ptoks.get(pos).map(|t| t.t.to_lowercase()).unwrap_or_default();
                if !bad.is_empty() && pos == 0 {
                    if let Some(sg) = super::suggest_cmd(&bad, &kwr) {
                        return result(format!("{}💡 Menade du {}?", self.err_at(st, line, pos + start, &toks), sg));
                    }
                }
                result(self.err_at(st, line, pos + start, &toks))
            }
            ParseRes::Ok { cmd, vals, kws, .. } => {
                if neg && !cmd.no {
                    return result(self.err_at(st, line, 0, &toks));
                }
                if !neg && cmd.no_only {
                    return result("% Incomplete command.\n");
                }
                let args = Args { v: vals, neg, kws, line: line.to_string() };
                (cmd.f)(self, st, &args)
            }
        }
    }

    // ---------- Hjälp (?) och tab
    pub fn help_line(&mut self, _st: &mut State, line: &str) -> String {
        if self.pending.is_some() {
            return String::new();
        }
        let tl = line.trim_end();
        if tl.ends_with('|') {
            return [
                "  append    Append redirected output to URL (URLs supporting append operation only)",
                "  begin     Begin with the line that matches",
                "  count     Count number of lines which match regexp",
                "  exclude   Exclude lines that match",
                "  include   Include lines that match",
                "  section   Filter a section of output",
            ]
            .join("\n");
        }
        if let Some(p) = line.rfind('|') {
            let after = &line[p + 1..];
            if line.ends_with(char::is_whitespace) && after.split_whitespace().count() == 1 {
                return "  LINE  Regular Expression".into();
            }
        }
        let tr = tries();
        let mode = self.mode.clone();
        let mut toks = tokenize(line);
        let trailing = line.is_empty() || line.ends_with(char::is_whitespace);
        let empty = Node::default();
        let mut root = tr.get(&mode).unwrap_or(&empty);
        let in_sub = mode != "user" && mode != "exec";
        if in_sub && !toks.is_empty() && toks[0].t.to_lowercase() == "no" && (toks.len() > 1 || trailing) {
            toks.remove(0);
        } else if in_sub && !toks.is_empty() && toks[0].t.to_lowercase() == "do" && (toks.len() > 1 || trailing) {
            toks.remove(0);
            root = tr.get("exec").unwrap_or(&empty);
        }
        let ctx_toks: Vec<Tok> = if trailing { toks.clone() } else { toks[..toks.len().saturating_sub(1)].to_vec() };
        let partial = if trailing { String::new() } else { toks.last().map(|t| t.t.to_lowercase()).unwrap_or_default() };
        let mut node = root;
        if !ctx_toks.is_empty() {
            match parse(root, &ctx_toks, line) {
                ParseRes::Incomplete { node: n } => node = n,
                ParseRes::Ok { node: n, .. } => node = n,
                _ => return "% Unrecognized command".into(),
            }
        }
        if !trailing {
            let mut ks: Vec<String> = node.kw_names().into_iter().filter(|k| k.starts_with(&partial)).collect();
            ks.sort();
            if ks.is_empty() {
                return "% Unrecognized command".into();
            }
            return ks.join("  ");
        }
        let mut rows = vec![];
        let in_show = !ctx_toks.is_empty() && "show".starts_with(&ctx_toks[0].t.to_lowercase()) && ctx_toks[0].t.len() >= 2;
        let mut names = node.kw_names();
        names.sort();
        for k in names {
            rows.push(format!("  {}{}", U::pad(&k, 22), super::ios_help::help_text(&k, in_show)));
        }
        for (spec, _) in &node.params {
            let inner = &spec[1..spec.len() - 1];
            let mut parts = inner.split(':');
            let ty = parts.next().unwrap_or("");
            if ty == "n" {
                let r = parts.next().unwrap_or("0-4294967295");
                rows.push(format!("  {}Number", U::pad(format!("<{}>", r), 22)));
            } else if let Some((a, b)) = super::ios_help::param_help(ty) {
                rows.push(format!("  {}{}", U::pad(a, 22), b));
            }
        }
        if node.cmd.is_some() {
            rows.push("  <cr>".into());
        }
        rows.join("\n")
    }

    pub fn complete_line(&mut self, st: &mut State, line: &str) -> Option<String> {
        let _ = st;
        let toks = tokenize(line);
        if toks.is_empty() || line.ends_with(char::is_whitespace) {
            return None;
        }
        let tr = tries();
        let empty = Node::default();
        let mut root = tr.get(&self.mode).unwrap_or(&empty);
        let mut prefix: Vec<Tok> = toks[..toks.len() - 1].to_vec();
        let in_sub = self.mode != "user" && self.mode != "exec";
        if in_sub && !prefix.is_empty() && prefix[0].t.to_lowercase() == "no" {
            prefix.remove(0);
        }
        if in_sub && !prefix.is_empty() && prefix[0].t.to_lowercase() == "do" {
            prefix.remove(0);
            root = tr.get("exec").unwrap_or(&empty);
        }
        let mut node = root;
        if !prefix.is_empty() {
            match parse(root, &prefix, line) {
                ParseRes::Ok { node: n, .. } | ParseRes::Incomplete { node: n } => node = n,
                _ => return None,
            }
        }
        let last_tok = toks.last().unwrap();
        let last = last_tok.t.to_lowercase();
        let ks: Vec<String> = node.kw_names().into_iter().filter(|k| k.starts_with(&last)).collect();
        if ks.is_empty() && last.chars().all(|c| c.is_ascii_lowercase()) && node.params.iter().any(|(p, _)| p == "<if>") {
            let types: Vec<&str> = ["GigabitEthernet", "Vlan", "Loopback"].into_iter().filter(|t| t.to_lowercase().starts_with(&last)).collect();
            if types.len() == 1 {
                return Some(format!("{}{}", &line[..last_tok.pos], types[0]));
            }
        }
        if ks.len() != 1 {
            return None;
        }
        Some(format!("{}{} ", &line[..last_tok.pos], ks[0]))
    }
}

fn find_pipe(line: &str) -> Option<usize> {
    // Första " | " med mellanslag före
    let b = line.as_bytes();
    for i in 1..b.len() {
        if b[i] == b'|' && b[i - 1].is_ascii_whitespace() && i + 1 < b.len() && b[i + 1].is_ascii_whitespace() {
            return Some(i);
        }
    }
    None
}

pub fn apply_pipe(out: &str, op: &str, arg: &str) -> String {
    let lines: Vec<&str> = out.split('\n').collect();
    let re = U::MiniRe::new(arg);
    if "include".starts_with(op) {
        return lines.into_iter().filter(|l| re.is_match(l)).collect::<Vec<_>>().join("\n");
    }
    if "exclude".starts_with(op) {
        return lines.into_iter().filter(|l| !re.is_match(l)).collect::<Vec<_>>().join("\n");
    }
    if "begin".starts_with(op) {
        return match lines.iter().position(|l| re.is_match(l)) {
            Some(i) => lines[i..].join("\n"),
            None => String::new(),
        };
    }
    if "count".starts_with(op) && op.len() >= 2 {
        return format!("Number of lines which match regexp = {}", lines.iter().filter(|l| re.is_match(l)).count());
    }
    if "section".starts_with(op) {
        let mut out2 = vec![];
        let mut in_sec = false;
        for l in lines {
            if l.chars().next().map(|c| !c.is_whitespace()).unwrap_or(false) {
                in_sec = re.is_match(l);
            }
            if in_sec {
                out2.push(l);
            }
        }
        return out2.join("\n");
    }
    "% Invalid input detected at '^' marker.".into()
}

impl Cli for IosSession {
    fn prompt_text(&self, st: &State) -> String {
        match &self.pending {
            Some(p) => p.prompt.clone(),
            None => self.prompt(st),
        }
    }
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        self.handle_line(st, line)
    }
    fn help(&mut self, st: &mut State, line: &str) -> String {
        self.help_line(st, line)
    }
    fn complete(&mut self, st: &mut State, line: &str) -> Option<String> {
        self.complete_line(st, line)
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
    fn booting(&self) -> bool {
        self.booting
    }
    fn finish_boot(&mut self, st: &mut State) {
        self.booting = false;
        S::refresh(st);
        S::push_log(st, &self.dev.clone(), "%SYS-5-RESTART: System restarted --");
    }
    fn interrupt(&mut self, st: &mut State, ctrl_z: bool) {
        self.pending = None;
        if ctrl_z && self.mode != "user" && self.mode != "exec" {
            self.mode = "exec".into();
            self.ctx = Ctx::default();
            let via = if self.via == "console" { "console by console" } else { "vty0 (192.168.1.200)" };
            S::push_log(st, &self.dev.clone(), &format!("%SYS-5-CONFIG_I: Configured from {}", via));
        }
    }
    fn as_ios(&mut self) -> Option<&mut IosSession> {
        Some(self)
    }
}
