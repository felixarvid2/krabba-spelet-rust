//! Wlc-sessionen. Port av js/cli/wlc.js (och tilläggen i js/cli/more7.js).
#![allow(unused_variables)]
use super::{Cli, CmdResult};
use crate::net::sim::State;

pub struct WlcSession {
    pub dev: String,
    pub closed: bool,
}

impl WlcSession {
    pub fn new(st: &State, dev: &str) -> WlcSession {
        WlcSession { dev: dev.to_string(), closed: false }
    }
}

impl Cli for WlcSession {
    fn prompt_text(&self, st: &State) -> String {
        ">".into()
    }
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        todo!("wlc::handle")
    }
    fn closed(&self) -> bool {
        self.closed
    }
    fn dev_id(&self) -> &str {
        &self.dev
    }
}
