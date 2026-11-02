//! Public, transient samples only. Never persist these or authentication data.
use serde::Deserialize;
use std::time::Instant;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Option<f64>,
    pub load_average_5: Option<f64>,
    pub load_average_15: Option<f64>,
    pub cpu_count: Option<u32>,
    pub cpu_usage_percent: Option<f64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub latency_ms: Option<f64>,
    pub processes: Vec<Process>,
    pub network_interfaces: Vec<Interface>,
    pub filesystems: Vec<Filesystem>,
    pub error: Option<String>,
}
#[derive(Clone, Deserialize)]
pub struct Process {
    pub memory_bytes: u64,
    pub cpu_usage_percent: f64,
    pub command: String,
}
#[derive(Clone, Deserialize)]
pub struct Interface {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}
#[derive(Clone, Deserialize)]
pub struct Filesystem {
    pub path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}
impl Snapshot {
    pub fn has_data(&self) -> bool {
        self.hostname.is_some()
            || self.os_name.is_some()
            || self.cpu_usage_percent.is_some()
            || self.memory_total_bytes.is_some()
            || self.disk_total_bytes.is_some()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Memory,
    Cpu,
    Command,
}
#[derive(Default)]
pub struct Samples {
    pub info: Option<Snapshot>,
    pub interface: Option<String>,
    pub download: f64,
    pub upload: f64,
    pub downloads: Vec<f64>,
    pub uploads: Vec<f64>,
    pub latencies: Vec<f64>,
    pub sort: Sort,
    pub ascending: bool,
    baseline: Option<(Instant, Interface)>,
}
impl Samples {
    pub fn pause(&mut self) {
        self.baseline = None;
    }
    pub fn select_interface(&mut self, name: String) {
        if self.interface.as_ref() == Some(&name) {
            return;
        }
        self.interface = Some(name);
        self.baseline = None;
        self.download = 0.;
        self.upload = 0.;
        self.downloads.clear();
        self.uploads.clear();
    }
    pub fn apply(&mut self, info: Snapshot, now: Instant) {
        let selected = self
            .interface
            .as_ref()
            .and_then(|selected| info.network_interfaces.iter().find(|i| &i.name == selected))
            .or_else(|| info.network_interfaces.first())
            .cloned();
        if self.interface.as_deref() != selected.as_ref().map(|i| i.name.as_str()) {
            self.interface = selected.as_ref().map(|i| i.name.clone());
            self.baseline = None;
            self.downloads.clear();
            self.uploads.clear();
        }
        let rates = selected
            .as_ref()
            .zip(self.baseline.as_ref())
            .and_then(|(next, (time, previous))| {
                let elapsed = now.saturating_duration_since(*time).as_secs_f64();
                (next.name == previous.name && elapsed > 0.).then(|| {
                    (
                        next.received_bytes.saturating_sub(previous.received_bytes) as f64
                            / elapsed,
                        next.transmitted_bytes
                            .saturating_sub(previous.transmitted_bytes)
                            as f64
                            / elapsed,
                    )
                })
            })
            .unwrap_or((0., 0.));
        self.download = rates.0;
        self.upload = rates.1;
        self.baseline = selected.map(|i| (now, i));
        if info.has_data() {
            append(&mut self.downloads, self.download);
            append(&mut self.uploads, self.upload);
            if let Some(value) = info.latency_ms {
                append(&mut self.latencies, value);
            }
        }
        self.info = Some(info);
    }
    pub fn select_sort(&mut self, sort: Sort) {
        if self.sort == sort {
            self.ascending = !self.ascending;
        } else {
            self.sort = sort;
            self.ascending = sort == Sort::Command;
        }
    }
    pub fn processes(&self) -> Vec<&Process> {
        let mut values: Vec<_> = self.info.iter().flat_map(|info| &info.processes).collect();
        values.sort_by(|a, b| {
            let order = match self.sort {
                Sort::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                Sort::Cpu => a.cpu_usage_percent.total_cmp(&b.cpu_usage_percent),
                Sort::Command => a.command.to_lowercase().cmp(&b.command.to_lowercase()),
            };
            (if self.ascending {
                order
            } else {
                order.reverse()
            })
            .then_with(|| a.command.to_lowercase().cmp(&b.command.to_lowercase()))
        });
        values
    }
}
fn append(values: &mut Vec<f64>, value: f64) {
    if !value.is_finite() || value < 0. {
        return;
    }
    values.push(value);
    if values.len() > 30 {
        values.remove(0);
    }
}
pub fn usage(used: Option<u64>, total: Option<u64>) -> Option<f32> {
    used.zip(total)
        .filter(|(_, total)| *total > 0)
        .map(|(used, total)| (used as f64 / total as f64).clamp(0., 1.) as f32)
}
pub fn bytes(value: u64) -> String {
    let mut number = value as f64;
    let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let mut unit = 0;
    while number >= 1024. && unit + 1 < units.len() {
        number /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{number:.1} {}", units[unit])
    }
}
pub fn used_total(used: Option<u64>, total: Option<u64>) -> String {
    used.zip(total)
        .map(|(used, total)| format!("{} / {}", bytes(used), bytes(total)))
        .unwrap_or_else(|| "—".into())
}
pub fn uptime(value: u64) -> String {
    format!(
        "{}d {}h {}m",
        value / 86400,
        value % 86400 / 3600,
        value % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::{Samples, Snapshot, Sort, usage};
    use std::time::{Duration, Instant};
    fn snapshot(received: u64, transmitted: u64, interface: &str) -> Snapshot {
        serde_json::from_value(serde_json::json!({
            "hostname":"synthetic", "network_interfaces":[{"name":interface,"received_bytes":received,"transmitted_bytes":transmitted}],
            "latency_ms": 10,
            "processes":[{"memory_bytes":20,"cpu_usage_percent":1,"command":"z"},{"memory_bytes":10,"cpu_usage_percent":2,"command":"a"}]
        })).unwrap()
    }
    #[test]
    fn network_rates_reset_on_pause_counter_reset_and_interface_change() {
        let now = Instant::now();
        let mut state = Samples::default();
        state.apply(snapshot(100, 200, "eth0"), now);
        assert_eq!((state.download, state.upload), (0., 0.));
        state.apply(snapshot(600, 1200, "eth0"), now + Duration::from_secs(5));
        assert_eq!((state.download, state.upload), (100., 200.));
        state.apply(snapshot(0, 0, "eth0"), now + Duration::from_secs(10));
        assert_eq!((state.download, state.upload), (0., 0.));
        state.pause();
        state.apply(
            snapshot(99999, 99999, "eth0"),
            now + Duration::from_secs(15),
        );
        assert_eq!((state.download, state.upload), (0., 0.));
        state.select_interface("eth1".into());
        state.apply(
            snapshot(999999, 999999, "eth1"),
            now + Duration::from_secs(20),
        );
        assert_eq!((state.download, state.upload), (0., 0.));
        assert_eq!(state.downloads.len(), 1);
    }
    #[test]
    fn histories_are_bounded_and_process_sort_and_usage_match_reference() {
        let now = Instant::now();
        let mut state = Samples::default();
        for i in 0..40 {
            state.apply(snapshot(i, i, "eth0"), now + Duration::from_secs(i));
        }
        assert_eq!(state.downloads.len(), 30);
        assert_eq!(state.latencies.len(), 30);
        assert_eq!(state.processes()[0].command, "z");
        state.select_sort(Sort::Cpu);
        assert_eq!(state.processes()[0].command, "a");
        state.select_sort(Sort::Cpu);
        assert_eq!(state.processes()[0].command, "z");
        assert_eq!(usage(Some(1), Some(0)), None);
        assert_eq!(usage(Some(20), Some(10)), Some(1.));
    }
}
