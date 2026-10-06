use anyhow::{bail, Context, Result};
use std::fs;

#[derive(Debug, PartialEq)]
pub struct MemoryInfo {
    pub available: u64,
    pub total: u64,
}

pub(super) fn read_memory() -> Result<MemoryInfo> {
    let raw = fs::read_to_string("/proc/meminfo").context("No se pudo leer /proc/meminfo")?;
    parse_memory(&raw)
}

fn parse_memory(raw: &str) -> Result<MemoryInfo> {
    let mut total = None;
    let mut available = None;
    for line in raw.lines() {
        let mut fields = line.split_whitespace();
        let target = match fields.next() {
            Some("MemTotal:") => &mut total,
            Some("MemAvailable:") => &mut available,
            _ => continue,
        };
        let kib: u64 = fields
            .next()
            .context("Falta el valor de RAM")?
            .parse()
            .context("Valor de RAM no válido")?;
        if fields.next() != Some("kB") {
            bail!("Unidad de RAM no válida");
        }
        *target = Some(
            kib.checked_mul(1024)
                .context("Valor de RAM fuera de rango")?,
        );
    }
    let total = total.context("Falta MemTotal en /proc/meminfo")?;
    let available = available.context("Falta MemAvailable en /proc/meminfo")?;
    if total == 0 || available > total {
        bail!("RAM disponible/total incoherente");
    }
    Ok(MemoryInfo { available, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_uses_available_not_free_and_converts_kib() {
        assert_eq!(
            parse_memory("MemTotal: 8388608 kB\nMemFree: 1024 kB\nMemAvailable: 4194304 kB\n")
                .unwrap(),
            MemoryInfo {
                available: 4 << 30,
                total: 8 << 30
            }
        );
    }

    #[test]
    fn invalid_memory_is_reported() {
        for raw in [
            "MemTotal: 8 kB",
            "MemTotal: 8 kB\nMemAvailable: 9 kB",
            "MemTotal: 0 kB\nMemAvailable: 0 kB",
            "MemTotal: bad kB\nMemAvailable: 0 kB",
            "MemTotal: 8 MB\nMemAvailable: 0 kB",
            "MemTotal: 18446744073709551615 kB\nMemAvailable: 0 kB",
        ] {
            assert!(parse_memory(raw).is_err(), "{raw}");
        }
    }
}
