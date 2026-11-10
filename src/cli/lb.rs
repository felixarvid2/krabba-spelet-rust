//! Lb-sessionen. Port av js/cli/lb.js (och tilläggen i js/cli/more7.js).
#![allow(unused_variables)]
use super::{Cli, CmdResult};
use crate::net::sim::State;

pub struct LbSession {
    pub dev: String,
    pub closed: bool,
}

impl LbSession {
    pub fn new(st: &State, dev: &str) -> LbSession {
        LbSession { dev: dev.to_string(), closed: false }
    }
}

impl Cli for LbSession {
    fn prompt_text(&self, st: &State) -> String {
        ">".into()
    }
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult {
        todo!("lb::handle")
    }
    fn closed(&self) -> bool {
        self.closed
    }
    fn dev_id(&self) -> &str {
        &self.dev
    }
}
