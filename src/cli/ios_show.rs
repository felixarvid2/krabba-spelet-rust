//! show-utdata för IOS. Port av js/cli/ios_show.js. Alla funktioner returnerar texten som IOS skulle skriva.
#![allow(unused_variables)]
use crate::net::model::{Ace, Device, IosConfig};
use crate::net::sim::State;

/// Filter för show mac address-table
#[derive(Clone, Debug, Default)]
pub struct MacFilter {
    pub address: Option<String>,
    pub iface: Option<String>,
    pub vlan: Option<u16>,
}

/// Interfacenamnen i den ordning IOS visar dem (ifaceNames)
pub fn iface_names(d: &Device) -> Vec<String> {
    todo!("ios_show::iface_names")
}
/// Fysiska portar (physPorts)
pub fn phys_ports(d: &Device) -> Vec<String> {
    todo!("ios_show::phys_ports")
}
/// "connected", "notconnect", "disabled", "err-disabled" ... (portState)
pub fn port_state(st: &mut State, dev: &str, n: &str) -> String {
    todo!("ios_show::port_state")
}
/// Texten för en ACL-rad, t.ex. "deny   ip 192.168.1.128 0.0.0.63 192.168.1.0 0.0.0.255" (aceText)
pub fn ace_text(extended: bool, r: &Ace) -> String {
    todo!("ios_show::ace_text")
}
/// Typ 9-hash av ett lösenord (hashSecret)
pub fn hash_secret(s: &str) -> String {
    todo!("ios_show::hash_secret")
}
/// Hur länge sedan rutten lärdes, t.ex. "00:12:31" (routeAge)
pub fn route_age(st: &mut State, dev: &str, kind: &str, net: &str) -> String {
    todo!("ios_show::route_age")
}

pub fn running_config(st: &mut State, dev: &str) -> String {
    todo!("ios_show::running_config")
}
/// Running-config-text för en given konfiguration (runningConfig(state, d, c))
pub fn running_config_of(st: &mut State, dev: &str, c: &IosConfig) -> String {
    todo!("ios_show::running_config_of")
}
pub fn startup_config(st: &mut State, dev: &str) -> String {
    todo!("ios_show::startup_config")
}
pub fn running_interface(st: &mut State, dev: &str, n: &str) -> String {
    todo!("ios_show::running_interface")
}
pub fn interfaces_status(st: &mut State, dev: &str, filter: Option<&str>) -> String {
    todo!("ios_show::interfaces_status")
}
pub fn ip_int_brief(st: &mut State, dev: &str) -> String {
    todo!("ios_show::ip_int_brief")
}
pub fn show_interface(st: &mut State, dev: &str, n: &str) -> String {
    todo!("ios_show::show_interface")
}
pub fn switchport(st: &mut State, dev: &str, n: &str) -> String {
    todo!("ios_show::switchport")
}
pub fn vlan_brief(st: &mut State, dev: &str) -> String {
    todo!("ios_show::vlan_brief")
}
pub fn interfaces_trunk(st: &mut State, dev: &str) -> String {
    todo!("ios_show::interfaces_trunk")
}
pub fn mac_table(st: &mut State, dev: &str, filt: MacFilter) -> String {
    todo!("ios_show::mac_table")
}
pub fn spanning_tree(st: &mut State, dev: &str, only_vlan: Option<u16>) -> String {
    todo!("ios_show::spanning_tree")
}
pub fn power_inline(st: &mut State, dev: &str, only: Option<&str>) -> String {
    todo!("ios_show::power_inline")
}
pub fn port_security(st: &mut State, dev: &str, only: Option<&str>) -> String {
    todo!("ios_show::port_security")
}
pub fn cdp_neighbors(st: &mut State, dev: &str) -> String {
    todo!("ios_show::cdp_neighbors")
}
/// filt: None, "static", "connected", "ospf", "bgp" eller en IP-adress
pub fn ip_route(st: &mut State, dev: &str, filt: Option<&str>) -> String {
    todo!("ios_show::ip_route")
}
pub fn dhcp_binding(st: &mut State, dev: &str) -> String {
    todo!("ios_show::dhcp_binding")
}
pub fn dhcp_pool(st: &mut State, dev: &str, only: Option<&str>) -> String {
    todo!("ios_show::dhcp_pool")
}
pub fn nat_translations(st: &mut State, dev: &str) -> String {
    todo!("ios_show::nat_translations")
}
pub fn nat_statistics(st: &mut State, dev: &str) -> String {
    todo!("ios_show::nat_statistics")
}
pub fn access_lists(st: &mut State, dev: &str, only: Option<&str>, ip_only: bool) -> String {
    todo!("ios_show::access_lists")
}
pub fn ip_interface(st: &mut State, dev: &str, n: &str) -> String {
    todo!("ios_show::ip_interface")
}
pub fn arp_table(st: &mut State, dev: &str) -> String {
    todo!("ios_show::arp_table")
}
pub fn ip_ssh(st: &mut State, dev: &str) -> String {
    todo!("ios_show::ip_ssh")
}
pub fn show_version(st: &mut State, dev: &str) -> String {
    todo!("ios_show::show_version")
}
pub fn show_clock(st: &mut State, dev: &str) -> String {
    todo!("ios_show::show_clock")
}
pub fn show_clock_detail(st: &mut State, dev: &str) -> String {
    todo!("ios_show::show_clock_detail")
}
pub fn ntp_associations(st: &mut State, dev: &str) -> String {
    todo!("ios_show::ntp_associations")
}
pub fn ntp_status(st: &mut State, dev: &str) -> String {
    todo!("ios_show::ntp_status")
}
pub fn ip_cef(st: &mut State, dev: &str, ip: &str) -> String {
    todo!("ios_show::ip_cef")
}
pub fn show_logging(st: &mut State, dev: &str) -> String {
    todo!("ios_show::show_logging")
}
pub fn interfaces_description(st: &mut State, dev: &str) -> String {
    todo!("ios_show::interfaces_description")
}
pub fn vlan_id(st: &mut State, dev: &str, v: u16) -> String {
    todo!("ios_show::vlan_id")
}
pub fn processes_cpu(st: &mut State, dev: &str) -> String {
    todo!("ios_show::processes_cpu")
}
pub fn cdp_detail(st: &mut State, dev: &str) -> String {
    todo!("ios_show::cdp_detail")
}
pub fn stp_summary(st: &mut State, dev: &str) -> String {
    todo!("ios_show::stp_summary")
}
pub fn stp_blocked(st: &mut State, dev: &str) -> String {
    todo!("ios_show::stp_blocked")
}
pub fn inventory(st: &mut State, dev: &str) -> String {
    todo!("ios_show::inventory")
}
pub fn counters_errors(st: &mut State, dev: &str) -> String {
    todo!("ios_show::counters_errors")
}
pub fn crypto_isakmp_sa(st: &mut State, dev: &str) -> String {
    todo!("ios_show::crypto_isakmp_sa")
}
pub fn crypto_ipsec_sa(st: &mut State, dev: &str) -> String {
    todo!("ios_show::crypto_ipsec_sa")
}
pub fn crypto_map_show(st: &mut State, dev: &str) -> String {
    todo!("ios_show::crypto_map_show")
}
pub fn crypto_isakmp_policy(st: &mut State, dev: &str) -> String {
    todo!("ios_show::crypto_isakmp_policy")
}
pub fn crypto_session(st: &mut State, dev: &str) -> String {
    todo!("ios_show::crypto_session")
}
