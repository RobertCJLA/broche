// SPDX-License-Identifier: GPL-3.0-or-later
//! The connection check (the original NectarLink's "Connection Doctor"): what stands between a
//! phone and this PC, worst first, in plain words, with the command to run when the fix needs
//! the administrator. It only reads: nothing here changes the firewall or the network, and it
//! never asks for a password (D-Bus calls go without interactive authorization).
use std::net::{IpAddr, SocketAddr};

use broche_core::{ConnectionPath, LinkState, Node};

use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

/// One line of the check, as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub title: String,
    pub detail: String,
    /// A command to copy and run in a terminal (it needs the administrator).
    pub command: Option<String>,
}

/// What the firewall (firewalld) says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firewall {
    /// firewalld isn't running (another firewall may be).
    Absent,
    /// It answered nothing usable.
    Unknown,
    /// The zone of the network, whether Broche's UDP port gets in, whether mDNS does.
    Zone { name: String, port_open: bool, mdns: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    pub name: String,
    pub link: Option<ConnectionPath>,
    /// Seconds since it was last heard on the local network, if it was.
    pub seen_ago: Option<u64>,
    pub failures: u32,
}

/// Everything the check looks at, gathered first so the wording can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub core: bool,
    /// This PC's addresses on a real local network (virtual bridges left out).
    pub lan: Vec<IpAddr>,
    /// Broche's UDP port, when known.
    pub port: Option<u16>,
    pub vpn: Vec<String>,
    pub firewall: Firewall,
    pub phones: Vec<Phone>,
}

/// Reads the facts (blocking parts on a worker thread).
pub async fn gather(node: Option<Node>) -> Facts {
    let Some(node) = node else {
        return Facts {
            core: false,
            lan: vec![],
            port: None,
            vpn: vpns(),
            firewall: Firewall::Unknown,
            phones: vec![],
        };
    };
    let addrs: Vec<SocketAddr> = node.direct_addrs().await;
    let port = addrs.first().map(SocketAddr::port);
    let lan = addrs.iter().map(SocketAddr::ip).filter(|ip| is_lan(*ip)).collect();
    let phones = node
        .paired_devices()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let d = node.peer_diagnostics(&p.id);
            Phone {
                name: p.display_name,
                link: match p.link {
                    LinkState::Online { path, .. } => Some(path),
                    _ => None,
                },
                seen_ago: d.mdns_seen_ago_secs,
                failures: d.dial_failures,
            }
        })
        .collect();
    let firewall = tokio::task::spawn_blocking(move || firewall(port)).await.unwrap_or(Firewall::Unknown);
    Facts { core: true, lan, port, vpn: vpns(), firewall, phones }
}

/// A private address on a real network: not loopback or link-local, and not the default
/// bridges of Docker (172.17/16), Podman (10.88/16) or libvirt (192.168.122/24).
/// shortcut: the bridges are told apart by their default ranges; a custom bridge counts as LAN.
fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                && !(o[0] == 172 && o[1] == 17)
                && !(o[0] == 10 && o[1] == 88)
                && !(o[0] == 192 && o[1] == 168 && o[2] == 122)
        }
        IpAddr::V6(_) => false,
    }
}

/// Network interfaces that look like a VPN.
fn vpns() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else { return vec![] };
    let mut found: Vec<String> = dir
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            ["tun", "tap", "wg", "ppp", "tailscale", "proton", "nordlynx"].iter().any(|p| n.starts_with(p))
        })
        .collect();
    found.sort();
    found
}

/// firewalld over D-Bus with `busctl`, on the host (a Flatpak can't reach the system bus), without
/// interactive authorization: if it ever needed a password, the call fails instead of asking.
fn firewall(port: Option<u16>) -> Firewall {
    let call = |args: &[&str]| -> Result<serde_json::Value, bool> {
        let out = crate::platform::host("busctl")
            .args(["--system", "--allow-interactive-authorization=no", "--json=short", "call"])
            .args(["org.fedoraproject.FirewallD1", "/org/fedoraproject/FirewallD1"])
            .args(args)
            .output()
            .map_err(|_| false)?;
        if !out.status.success() {
            // Not there at all, as opposed to there but unhelpful.
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(err.contains("not activatable")
                || err.contains("was not provided")
                || err.contains("ServiceUnknown"));
        }
        serde_json::from_slice(&out.stdout).map_err(|_| false)
    };
    let zones = match call(&["org.fedoraproject.FirewallD1.zone", "getActiveZones"]) {
        Ok(v) => v,
        Err(true) => return Firewall::Absent,
        Err(false) => return Firewall::Unknown,
    };
    // The zone that has interfaces (the network in use), else the default one.
    let zone = zones["data"][0]
        .as_object()
        .and_then(|m| {
            m.iter()
                .find(|(_, z)| z["interfaces"].as_array().is_some_and(|i| !i.is_empty()))
                .map(|(name, _)| name.clone())
        })
        .or_else(|| {
            call(&["org.fedoraproject.FirewallD1", "getDefaultZone"]).ok()?["data"][0]
                .as_str()
                .map(String::from)
        });
    let Some(name) = zone else { return Firewall::Unknown };
    let ports = call(&["org.fedoraproject.FirewallD1.zone", "getPorts", "s", &name]).ok();
    let open: Vec<(String, String)> = ports
        .as_ref()
        .and_then(|v| v["data"][0].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string())))
        .collect();
    let services: Vec<String> = call(&["org.fedoraproject.FirewallD1.zone", "getServices", "s", &name])
        .ok()
        .and_then(|v| v["data"][0].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.as_str().map(String::from))
        .collect();
    let port_open = port.is_some_and(|port| udp_allows(&open, port));
    Firewall::Zone { name, port_open, mdns: services.iter().any(|s| s == "mdns") || udp_allows(&open, 5353) }
}

/// Whether firewalld's port list (`"40000-41000"`/`"udp"`, `"5353"`/`"udp"`…) lets `port` in.
fn udp_allows(ports: &[(String, String)], port: u16) -> bool {
    ports.iter().filter(|(_, proto)| proto == "udp").any(|(range, _)| {
        let (lo, hi) = range.split_once('-').unwrap_or((range, range));
        matches!((lo.parse::<u16>(), hi.parse::<u16>()), (Ok(lo), Ok(hi)) if (lo..=hi).contains(&port))
    })
}

/// The findings, worst first.
pub fn findings(f: &Facts) -> Vec<Finding> {
    let mut out = vec![];
    let mut add = |level, title: String, detail: String, command: Option<String>| {
        out.push(Finding { level, title, detail, command });
    };
    if !f.core {
        add(
            Level::Fail,
            t("Broche's core isn't running"),
            t(
                "Another copy of Broche may be using the same data, or the keyring didn't answer. Close other copies and open Broche again.",
            ),
            None,
        );
    } else {
        add(Level::Ok, t("Broche's core is running"), String::new(), None);
    }
    match f.lan.first() {
        Some(ip) if f.core => add(Level::Ok, t("This PC is on a local network"), ip.to_string(), None),
        None if f.core => add(
            Level::Fail,
            t("This PC has no local network address"),
            t("Connect it to Wi-Fi or a cable, on the same network as the phone."),
            None,
        ),
        _ => {}
    }
    if !f.vpn.is_empty() {
        add(
            Level::Warn,
            format!("{} ({})", t("A VPN is on"), f.vpn.join(", ")),
            t("It can keep the phone and this PC apart. If they don't connect, try with the VPN off."),
            None,
        );
    }
    match &f.firewall {
        Firewall::Absent => {
            add(Level::Ok, t("No firewall (firewalld) is blocking Broche"), String::new(), None)
        }
        Firewall::Unknown => add(Level::Warn, t("The firewall couldn't be checked"), String::new(), None),
        Firewall::Zone { name, port_open, mdns } => {
            match (port_open, f.port) {
                (true, _) => add(
                    Level::Ok,
                    t("The firewall lets the phone connect"),
                    format!("{} {name}", t("Zone:")),
                    None,
                ),
                (false, Some(port)) => add(
                    Level::Fail,
                    t("The firewall blocks the phone"),
                    t("Run this in a terminal (it asks for your password there):"),
                    Some(format!(
                        "sudo firewall-cmd --permanent --zone={name} --add-port={port}/udp && sudo firewall-cmd --reload"
                    )),
                ),
                (false, None) => {}
            }
            if !mdns {
                add(
                    Level::Warn,
                    t("This PC can't hear phones announce themselves"),
                    t("Phones still find this PC. For this PC to find them too, run this in a terminal:"),
                    Some(format!(
                        "sudo firewall-cmd --permanent --zone={name} --add-service=mdns && sudo firewall-cmd --reload"
                    )),
                );
            }
        }
    }
    if f.core && f.phones.is_empty() {
        add(Level::Warn, t("No phone is paired"), t("Pair one in Settings › Pair."), None);
    }
    for p in &f.phones {
        match (p.link, p.seen_ago) {
            (Some(ConnectionPath::Relay), _) => add(
                Level::Warn,
                format!("{}: {}", p.name, t("connected through the internet")),
                t("It works, but slower. On the same Wi-Fi it connects directly."),
                None,
            ),
            (Some(_), _) => add(Level::Ok, format!("{}: {}", p.name, t("connected")), String::new(), None),
            (None, Some(ago)) if ago < 120 => add(
                Level::Fail,
                format!("{}: {}", p.name, t("on this network but not connecting")),
                format!(
                    "{} · {}",
                    t("Seen {} s ago").replace("{}", &ago.to_string()),
                    t("{} failed attempts").replace("{}", &p.failures.to_string())
                ),
                None,
            ),
            (None, _) => add(
                Level::Warn,
                format!("{}: {}", p.name, t("not seen on this network")),
                t(
                    "Open Broche on the phone and check it uses the same Wi-Fi. Some routers keep devices apart (client isolation).",
                ),
                None,
            ),
        }
    }
    out.sort_by_key(|f| match f.level {
        Level::Fail => 0,
        Level::Warn => 1,
        Level::Ok => 2,
    });
    out
}

impl crate::App {
    /// Runs the connection check in the background; its findings arrive as `Msg::Doctor`.
    pub(crate) fn run_doctor(&mut self, ctx: &eframe::egui::Context) {
        if self.doctor_running {
            return;
        }
        self.doctor_running = true;
        let (node, tx, ctx) = (self.node.clone(), self.tx.clone(), ctx.clone());
        self.rt.spawn(async move {
            let found = findings(&gather(node).await);
            let _ = tx.send(crate::Msg::Doctor(found));
            ctx.request_repaint();
        });
    }

    /// Settings › Device: the connection check, run on request.
    pub(crate) fn doctor_card(&mut self, ui: &mut eframe::egui::Ui) {
        use crate::{kit, theme::pal};
        use eframe::egui::RichText;
        kit::card(ui, |ui| {
            kit::title(ui, &t("Connection check"));
            kit::supporting(ui, t("What could keep a phone from reaching this PC, and how to fix it."));
            ui.add_space(4.0);
            let label = if self.doctor_running { t("Checking…") } else { t("Check connection") };
            if ui.add_enabled_ui(!self.doctor_running, |ui| kit::filled_button(ui, &label)).inner.clicked() {
                self.run_doctor(ui.ctx());
            }
            let Some(found) = self.doctor.clone() else { return };
            ui.add_space(4.0);
            let p = pal(ui);
            for (i, f) in found.iter().enumerate() {
                if i > 0 {
                    kit::divider(ui);
                }
                let (colour, word) = match f.level {
                    Level::Ok => (p.success, t("OK")),
                    Level::Warn => (p.warning, t("Check")),
                    Level::Fail => (p.error, t("Problem")),
                };
                ui.horizontal(|ui| {
                    kit::dot(ui, colour);
                    // The level in words too, not only by colour.
                    ui.label(RichText::new(format!("{word}: {}", f.title)).color(p.on_surface));
                });
                if !f.detail.is_empty() {
                    kit::supporting(ui, f.detail.clone());
                }
                if let Some(command) = &f.command {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(command).monospace().color(p.on_surface_variant));
                        if kit::tonal_button(ui, &t("Copy command")).clicked() {
                            ui.ctx().copy_text(command.clone());
                        }
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Facts {
        Facts {
            core: true,
            lan: vec!["192.168.1.20".parse().unwrap()],
            port: Some(40567),
            vpn: vec![],
            firewall: Firewall::Zone { name: "FedoraWorkstation".into(), port_open: true, mdns: true },
            phones: vec![Phone {
                name: "Pixel".into(),
                link: Some(ConnectionPath::Lan),
                seen_ago: Some(3),
                failures: 0,
            }],
        }
    }

    #[test]
    fn a_healthy_pc_has_only_good_news() {
        assert!(findings(&healthy()).iter().all(|f| f.level == Level::Ok));
    }

    #[test]
    fn worst_first_with_the_command_to_open_the_firewall() {
        let mut f = healthy();
        f.firewall = Firewall::Zone { name: "home".into(), port_open: false, mdns: false };
        f.phones[0].link = None;
        f.phones[0].failures = 4;
        let out = findings(&f);
        assert_eq!(out[0].level, Level::Fail);
        assert!(out.iter().any(|x| x.command.as_deref().is_some_and(|c| c.contains("--add-port=40567/udp"))));
        assert!(out.iter().any(|x| x.command.as_deref().is_some_and(|c| c.contains("--add-service=mdns"))));
        // Nothing good comes before something bad.
        let levels: Vec<Level> = out.iter().map(|x| x.level).collect();
        assert!(levels.windows(2).all(|w| !(w[0] == Level::Ok && w[1] != Level::Ok)));
    }

    #[test]
    fn docker_and_podman_bridges_are_not_a_network() {
        assert!(is_lan("192.168.1.50".parse().unwrap()));
        assert!(!is_lan("172.17.0.1".parse().unwrap()));
        assert!(!is_lan("10.88.0.1".parse().unwrap()));
        assert!(!is_lan("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn firewalld_port_ranges() {
        let ports = vec![("1025-65535".to_string(), "udp".to_string()), ("22".into(), "tcp".into())];
        assert!(udp_allows(&ports, 40567));
        assert!(!udp_allows(&ports, 5353 - 5000));
        assert!(!udp_allows(&[("22".into(), "tcp".into())], 22));
    }
}
