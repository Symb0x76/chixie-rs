//! Find wireless boards: ARP table → probe `http://<ip>/description.xml` (UPnP).

use std::net::Ipv4Addr;
use std::process::Command;
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;

use crate::Result;

#[derive(Debug, Clone)]
pub struct Device {
    pub ip: Ipv4Addr,
    pub friendly_name: Option<String>,
    pub model_name: Option<String>,
}

/// IPv4 addresses from `arp -a`, skipping broadcast/multicast.
pub fn arp_ips() -> Result<Vec<Ipv4Addr>> {
    let out = Command::new("arp").arg("-a").output()?;
    Ok(parse_arp(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_arp(text: &str) -> Vec<Ipv4Addr> {
    let mut ips: Vec<Ipv4Addr> = text
        .lines()
        .filter_map(|l| l.split_whitespace().next()?.trim_matches(['(', ')']).parse().ok())
        .filter(|ip: &Ipv4Addr| !ip.is_multicast() && !ip.is_broadcast() && ip.octets()[3] != 255)
        .collect();
    ips.sort();
    ips.dedup();
    ips
}

fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(xml[start..end].trim())
}

pub fn probe(http: &Client, ip: Ipv4Addr) -> Option<Device> {
    let xml = http.get(format!("http://{ip}/description.xml")).send().ok()?.text().ok()?;
    if !xml.contains("urn:schemas-upnp-org:device-1-0") {
        return None;
    }
    Some(Device {
        ip,
        friendly_name: tag(&xml, "friendlyName").map(str::to_owned),
        model_name: tag(&xml, "modelName").map(str::to_owned),
    })
}

/// Probe all ARP neighbours in parallel.
pub fn discover() -> Result<Vec<Device>> {
    let http = Client::builder().timeout(Duration::from_millis(1500)).build()?;
    let ips = arp_ips()?;
    Ok(thread::scope(|s| {
        let hs: Vec<_> = ips.iter().map(|&ip| s.spawn({ let h = &http; move || probe(h, ip) })).collect();
        hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_arp() {
        let t = "Interface: 192.168.1.5 --- 0x7\n  Internet Address      Physical Address      Type\n  192.168.1.1           aa-bb-cc-dd-ee-ff     dynamic\n  192.168.1.255         ff-ff-ff-ff-ff-ff     static\n  224.0.0.22            01-00-5e-00-00-16     static\n";
        assert_eq!(parse_arp(t), vec![Ipv4Addr::new(192, 168, 1, 1)]);
    }

    #[test]
    fn extracts_tag() {
        assert_eq!(tag("<root><friendlyName> grbl </friendlyName></root>", "friendlyName"), Some("grbl"));
    }
}
