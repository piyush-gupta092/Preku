//! LAN discovery (Architecture.md §4.2): the PC advertises `_preku._tcp.local.` with its id, so
//! a paired phone finds it again when the PC's IP address changes. Also picks the LAN address
//! that goes into the pairing QR.

use std::net::{IpAddr, Ipv4Addr};

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::foundation::errors::{PrekuError, Result};

pub const SERVICE_TYPE: &str = "_preku._tcp.local.";

pub struct Advertiser {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertiser {
    pub fn start(pc_id: &str, pc_name: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(mdns_error)?;
        // Instance names must be unique on the network; the id guarantees that.
        let short_id: String = pc_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
        let instance = format!("{} {short_id}", sanitize(pc_name));
        let host = format!("preku-{short_id}.local.");
        let props = [("id", pc_id), ("v", "1")];
        let info = ServiceInfo::new(SERVICE_TYPE, &instance, &host, "", port, &props[..])
            .map_err(mdns_error)?
            .enable_addr_auto();
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(mdns_error)?;
        Ok(Self { daemon, fullname })
    }

    pub fn stop(self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

fn mdns_error(err: mdns_sd::Error) -> PrekuError {
    PrekuError::Bridge(format!("mdns: {err}"))
}

fn sanitize(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .take(40)
        .collect();
    if clean.trim().is_empty() {
        "Preku".to_owned()
    } else {
        clean.trim().to_owned()
    }
}

/// The address to put in the QR: a private IPv4 on this PC (192.168.x preferred, as home Wi-Fi
/// usually is), else any non-loopback IPv4.
pub fn lan_address() -> Option<Ipv4Addr> {
    let addrs: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .ok()?
        .into_iter()
        .filter(|iface| !iface.is_loopback())
        .filter_map(|iface| match iface.ip() {
            IpAddr::V4(v4) if !v4.is_link_local() => Some(v4),
            _ => None,
        })
        .collect();
    pick_lan_address(&addrs)
}

/// Pure: chooses among candidate addresses (tested).
pub fn pick_lan_address(addrs: &[Ipv4Addr]) -> Option<Ipv4Addr> {
    let rank = |ip: &Ipv4Addr| match ip.octets() {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [172, b, ..] if (16..=31).contains(&b) => 2,
        _ => 3,
    };
    addrs.iter().copied().min_by_key(rank)
}

/// This PC's name for the phone ("Preku" if the OS doesn't say).
pub fn pc_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|n| sanitize(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Preku".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_home_wifi_addresses() {
        let a = |s: &str| s.parse::<Ipv4Addr>().unwrap();
        assert_eq!(
            pick_lan_address(&[a("172.20.0.5"), a("192.168.1.20"), a("10.0.0.2")]),
            Some(a("192.168.1.20"))
        );
        assert_eq!(
            pick_lan_address(&[a("10.0.0.2"), a("8.8.8.8")]),
            Some(a("10.0.0.2"))
        );
        assert_eq!(pick_lan_address(&[]), None);
    }

    #[test]
    fn names_are_safe_for_mdns() {
        assert_eq!(sanitize("DESK-PC.local!"), "DESK-PClocal");
        assert_eq!(sanitize("..."), "Preku");
    }
}
