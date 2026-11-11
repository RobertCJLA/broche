// SPDX-License-Identifier: GPL-3.0-or-later
//! The programs this app runs for some features (FFmpeg, PipeWire tools, GStreamer…): which
//! are missing here, and the command that installs them on this distribution. Shown in About.

use std::{
    ffi::OsStr,
    process::{Command, Stdio},
};

/// Package names per distribution family: Fedora, Debian/Ubuntu, Arch, openSUSE.
/// shortcut: only the Fedora names were checked on a real system; check the others when
/// testing on those distributions.
type Packages = [&'static str; 4];

/// One feature and what it needs.
pub struct Need {
    /// The feature, as a `t()` key.
    pub feature: &'static str,
    /// Any one of these programs is enough.
    programs: &'static [&'static str],
    /// GStreamer elements it also needs (all of them).
    gst: &'static [&'static str],
    packages: Packages,
}

const PIPEWIRE: Packages = ["pipewire-utils", "pipewire-bin", "pipewire", "pipewire-tools"];

pub const NEEDS: &[Need] = &[
    Need {
        feature: "See the phone's screen and camera",
        programs: &["ffmpeg"],
        gst: &[],
        packages: ["ffmpeg-free", "ffmpeg", "ffmpeg", "ffmpeg"],
    },
    Need {
        feature: "Hear the phone's sound",
        programs: &["pw-play", "ffplay"],
        gst: &[],
        packages: PIPEWIRE,
    },
    Need { feature: "Phone as speaker", programs: &["pw-record", "parec"], gst: &[], packages: PIPEWIRE },
    Need { feature: "Find my PC (ring)", programs: &["ffplay", "pw-play"], gst: &[], packages: PIPEWIRE },
    Need {
        feature: "PC volume",
        programs: &["wpctl", "pactl"],
        gst: &[],
        packages: ["wireplumber", "wireplumber", "wireplumber", "wireplumber"],
    },
    Need {
        feature: "Share this PC's screen",
        programs: &["gst-launch-1.0"],
        gst: &["pipewiresrc", "h264"],
        packages: [
            "gstreamer1 pipewire-gstreamer gstreamer1-plugin-openh264",
            "gstreamer1.0-tools gstreamer1.0-pipewire gstreamer1.0-plugins-ugly",
            "gstreamer gst-plugin-pipewire gst-plugins-ugly",
            "gstreamer-utils gstreamer-plugin-pipewire gstreamer-plugins-ugly",
        ],
    },
    Need {
        feature: "Open apps from the shortcuts",
        programs: &["gio"],
        gst: &[],
        packages: ["glib2", "libglib2.0-bin", "glib2", "glib2-tools"],
    },
    Need {
        feature: "Open links and folders",
        programs: &["xdg-open"],
        gst: &[],
        packages: ["xdg-utils", "xdg-utils", "xdg-utils", "xdg-utils"],
    },
];

/// What is missing on this PC (looked up when called: it runs `gst-inspect-1.0`).
pub fn missing() -> Vec<&'static Need> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    NEEDS.iter().filter(|n| !available(&path, n)).collect()
}

fn available(path: &OsStr, need: &Need) -> bool {
    need.programs.iter().any(|p| found_in(path, p))
        && need
            .gst
            .iter()
            .all(|e| if *e == "h264" { crate::platform::has_h264_encoder() } else { gst_has(e) })
}

/// Whether `program` is an executable file in one of the `PATH` folders.
fn found_in(path: &OsStr, program: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .any(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

fn gst_has(element: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .args(["--exists", element])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The command that installs everything in `missing`, for this distribution, or `None` if
/// the distribution isn't one we know (then the program names are shown instead).
pub fn install_command(missing: &[&Need]) -> Option<String> {
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    command_for(&os, missing)
}

fn command_for(os_release: &str, missing: &[&Need]) -> Option<String> {
    let ids: Vec<&str> = os_release
        .lines()
        .filter_map(|l| l.strip_prefix("ID=").or_else(|| l.strip_prefix("ID_LIKE=")))
        .flat_map(|v| v.trim_matches('"').split_whitespace())
        .collect();
    let is = |names: &[&str]| ids.iter().any(|id| names.contains(id));
    let (column, tool) = if is(&["fedora", "rhel", "centos"]) {
        (0, "sudo dnf install")
    } else if is(&["debian", "ubuntu"]) {
        (1, "sudo apt install")
    } else if is(&["arch"]) {
        (2, "sudo pacman -S")
    } else if is(&["suse", "opensuse"]) {
        (3, "sudo zypper install")
    } else {
        return None;
    };
    let mut packages: Vec<&str> = Vec::new();
    for need in missing {
        for p in need.packages[column].split_whitespace() {
            if !packages.contains(&p) {
                packages.push(p);
            }
        }
    }
    (!packages.is_empty()).then(|| format!("{tool} {}", packages.join(" ")))
}

/// The programs a feature can use, for distributions we don't know.
pub fn programs(need: &Need) -> String {
    need.programs.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_found_only_when_executable_in_path() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("broche-deps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tool = dir.join("tool");
        std::fs::write(&tool, "#!/bin/sh\n").unwrap();
        let path = std::env::join_paths(["/nonexistent".into(), dir.clone()]).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!found_in(&path, "tool"), "not executable");
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(found_in(&path, "tool"));
        assert!(!found_in(&path, "other"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn install_command_matches_the_distribution_and_merges_packages() {
        let all: Vec<&Need> = NEEDS.iter().collect();
        let fedora = command_for("NAME=\"Fedora Linux\"\nID=fedora\n", &all[1..4]).unwrap();
        assert_eq!(fedora, "sudo dnf install pipewire-utils", "three features, one package");
        let mint = command_for("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n", &all[..1]).unwrap();
        assert_eq!(mint, "sudo apt install ffmpeg");
        assert!(command_for("ID=arch\n", &all[5..6]).unwrap().starts_with("sudo pacman -S gstreamer "));
        assert_eq!(command_for("ID=nixos\n", &all[..1]), None);
        assert_eq!(command_for("ID=fedora\n", &[]), None);
    }
}
