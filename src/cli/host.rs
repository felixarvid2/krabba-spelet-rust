//! Terminaler på datorerna: Windows (svensk kommandotolk) och teknikerns Linux-laptop.
//! Port av js/cli/host.js och js/cli/host_more.js.
#![allow(unused_variables)]
use super::{Cli, CmdResult};
use crate::net::sim::State;

/// En session på en värd (Windows-dator, Linux-laptop, server).
pub struct HostSession {
    pub dev: String,
    pub closed: bool,
}

impl HostSession {
    /// Ny session på värden (os avgör Windows eller Linux).
    pub fn new(st: &State, dev: &str) -> HostSession {
        HostSession { dev: dev.to_string(), closed: false }
    }
    /// Text som visas när terminalen öppnas (t.ex. "Microsoft Windows [Version ...]")
    pub fn banner(&self, st: &State) -> String {
        String::new()
    }
}

impl Cli for HostSession {
    fn prompt_text(&self, st: &State) -> String {
        "$ ".into()
    }
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        todo!("host::handle")
    }
    fn closed(&self) -> bool {
        self.closed
    }
    fn dev_id(&self) -> &str {
        &self.dev
    }
}
