// SPDX-License-Identifier: GPL-3.0-or-later
//! Low-frequency aggregate counters; no subprocesses, process lists or extra permissions.
use broche_core::{Node, Resources};
use std::{fs, time::Duration};

fn cpu_sample(text: &str) -> Option<(u64, u64)> {
    let mut fields = text.lines().next()?.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let values: Vec<u64> = fields.take(8).map(str::parse).collect::<Result<_, _>>().ok()?;
    if values.len() < 4 {
        return None;
    }
    Some((values.iter().sum(), values[3] + values.get(4).copied().unwrap_or(0)))
}
fn memory_percent(text: &str) -> Option<u8> {
    let field = |key: &str| {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name == key).then(|| value.split_whitespace().next()?.parse::<u64>().ok()).flatten()
        })
    };
    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    (total > 0).then(|| (100 - available.min(total) * 100 / total) as u8)
}
fn temperature() -> Option<i16> {
    // Only identified CPU sensors; a missing sensor is not a temperature of zero.
    for entry in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let dir = entry.path();
        let name = fs::read_to_string(dir.join("name")).unwrap_or_default();
        if !matches!(name.trim(), "coretemp" | "k10temp" | "zenpower") {
            continue;
        }
        let value = fs::read_to_string(dir.join("temp1_input")).ok()?.trim().parse::<i64>().ok()? / 1000;
        if (-40..=150).contains(&value) {
            return Some(value as i16);
        }
    }
    None
}
pub async fn report(node: Node) {
    let mut previous = None;
    loop {
        let current = fs::read_to_string("/proc/stat").ok().and_then(|s| cpu_sample(&s));
        if let (Some((total, idle)), Some((old_total, old_idle))) = (current, previous)
            && let Some(idle_percent) =
                (idle.saturating_sub(old_idle) * 100).checked_div(total.saturating_sub(old_total))
            && let Some(memory) = fs::read_to_string("/proc/meminfo").ok().and_then(|s| memory_percent(&s))
        {
            node.update_resources(Resources {
                cpu_percent: (100 - idle_percent.min(100)) as u8,
                memory_percent: memory,
                temperature_c: temperature(),
            })
            .await;
        }
        previous = current;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_counters() {
        assert_eq!(cpu_sample("cpu 10 0 20 50 5 0 0 0 99 99\n"), Some((85, 55)));
        assert_eq!(cpu_sample("cpu invalid"), None);
        assert_eq!(memory_percent("MemTotal: 1000 kB\nMemAvailable: 250 kB"), Some(75));
        assert_eq!(memory_percent("MemTotal: 0 kB\nMemAvailable: 0 kB"), None);
    }
}
