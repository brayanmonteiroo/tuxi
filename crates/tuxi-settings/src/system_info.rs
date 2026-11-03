//! Informação do sistema para a página Sobre.

use std::fs;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub pretty_name: String,
    pub kernel: String,
    pub cpu_model: String,
    pub memory_total_mb: u64,
}

impl SystemInfo {
    pub(crate) fn gather() -> Self {
        let os = parse_os_release(&read_lossy("/etc/os-release"));
        let mem_kb = parse_meminfo_total_kb(&read_lossy("/proc/meminfo"));
        Self {
            hostname: read_lossy("/etc/hostname").trim().to_string(),
            os_name: os
                .get("NAME")
                .cloned()
                .unwrap_or_else(|| "Linux".into()),
            os_version: os
                .get("VERSION")
                .or_else(|| os.get("VERSION_ID"))
                .cloned()
                .unwrap_or_default(),
            pretty_name: os
                .get("PRETTY_NAME")
                .cloned()
                .unwrap_or_else(|| "Linux".into()),
            kernel: read_lossy("/proc/sys/kernel/osrelease")
                .trim()
                .to_string(),
            cpu_model: parse_cpu_model(&read_lossy("/proc/cpuinfo")),
            memory_total_mb: mem_kb / 1024,
        }
    }

    pub(crate) fn memory_label(&self) -> String {
        if self.memory_total_mb >= 1024 {
            format!("{:.1} GiB", self.memory_total_mb as f64 / 1024.0)
        } else {
            format!("{} MiB", self.memory_total_mb)
        }
    }
}

fn read_lossy(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Parseia `/etc/os-release` (chave=valor, aspas opcionais).
pub(crate) fn parse_os_release(contents: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        map.insert(key.trim().to_string(), value);
    }
    map
}

pub(crate) fn parse_meminfo_total_kb(contents: &str) -> u64 {
    for line in contents.lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
        return digits.parse().unwrap_or(0);
    }
    0
}

pub(crate) fn parse_cpu_model(contents: &str) -> String {
    for line in contents.lines() {
        if let Some(model) = line.strip_prefix("model name") {
            let model = model.trim().trim_start_matches(':').trim().to_string();
            if !model.is_empty() {
                return model;
            }
        }
    }
    // ARM / outros
    for line in contents.lines() {
        if let Some(model) = line.strip_prefix("Hardware") {
            let model = model.trim().trim_start_matches(':').trim().to_string();
            if !model.is_empty() {
                return model;
            }
        }
    }
    "Desconhecido".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_basic() {
        let raw = r#"
NAME="Fedora Linux"
VERSION="42 (Workstation Edition)"
ID=fedora
VERSION_ID=42
PRETTY_NAME="Fedora Linux 42 (Workstation Edition)"
"#;
        let map = parse_os_release(raw);
        assert_eq!(map.get("NAME").unwrap(), "Fedora Linux");
        assert_eq!(map.get("VERSION_ID").unwrap(), "42");
        assert!(map.get("PRETTY_NAME").unwrap().contains("Fedora"));
    }

    #[test]
    fn meminfo_total() {
        let raw = "MemTotal:       16318456 kB\nMemFree: 123\n";
        assert_eq!(parse_meminfo_total_kb(raw), 16_318_456);
    }

    #[test]
    fn cpu_model_x86() {
        let raw = "processor\t: 0\nmodel name\t: AMD Ryzen 7\nvendor_id\t: AuthenticAMD\n";
        assert_eq!(parse_cpu_model(raw), "AMD Ryzen 7");
    }
}
