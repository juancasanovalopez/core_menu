use anyhow::{bail, Context, Result};
use std::{collections::HashSet, fs};

#[derive(Debug, PartialEq)]
pub struct CpuInfo {
    pub cores: usize,
    pub threads: usize,
}

fn parse_cpu_list(raw: &str) -> Result<HashSet<u32>> {
    let mut cpus = HashSet::new();
    for part in raw.trim().split(',') {
        let (start, end) = match part.split_once('-') {
            Some((start, end)) => (start.parse::<u32>()?, end.parse::<u32>()?),
            None => {
                let cpu = part.parse::<u32>()?;
                (cpu, cpu)
            }
        };
        if start > end {
            bail!("Rango de CPU no válido");
        }
        cpus.extend(start..=end);
    }
    Ok(cpus)
}

pub(super) fn read_cpu() -> Result<CpuInfo> {
    let online = fs::read_to_string("/sys/devices/system/cpu/online")
        .context("No se pudo leer la lista de CPU activas")?;
    count_cpu(&online, |cpu| {
        let path = format!("/sys/devices/system/cpu/cpu{cpu}/topology/core_cpus_list");
        fs::read_to_string(&path).with_context(|| format!("No se pudo leer {path}"))
    })
}

fn count_cpu(
    online: &str,
    mut read_siblings: impl FnMut(u32) -> Result<String>,
) -> Result<CpuInfo> {
    let cpus = parse_cpu_list(online).context("Lista de CPU activas no válida")?;
    let mut cores = HashSet::new();
    for &cpu in &cpus {
        let siblings =
            parse_cpu_list(&read_siblings(cpu)?).context("Topología de núcleos no válida")?;
        if !siblings.contains(&cpu) {
            bail!("La topología no contiene la CPU {cpu}");
        }
        // Each sibling group identifies one physical core, across all sockets.
        let mut siblings: Vec<_> = siblings.into_iter().collect();
        siblings.sort_unstable();
        cores.insert(siblings);
    }
    Ok(CpuInfo {
        cores: cores.len(),
        threads: cpus.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_physical_cores_separately_from_threads() {
        let cpu = count_cpu("0-3,8-11\n", |cpu| {
            let core = cpu % 4;
            Ok(format!("{core},{}", core + 8))
        })
        .unwrap();
        assert_eq!(
            cpu,
            CpuInfo {
                cores: 4,
                threads: 8
            }
        );
    }

    #[test]
    fn handles_no_smt_and_offline_siblings() {
        assert_eq!(
            count_cpu("0-3", |cpu| Ok(cpu.to_string())).unwrap(),
            CpuInfo {
                cores: 4,
                threads: 4
            }
        );
        assert_eq!(
            count_cpu("0,2", |cpu| Ok(format!("{cpu}-{}", cpu + 1))).unwrap(),
            CpuInfo {
                cores: 2,
                threads: 2
            }
        );
    }

    #[test]
    fn invalid_or_unreadable_topology_is_reported() {
        for raw in ["", "3-1", "bad", "0,"] {
            assert!(parse_cpu_list(raw).is_err());
        }
        assert!(count_cpu("0", |_| Ok("1".into())).is_err());
        assert!(count_cpu("0", |_| bail!("No se pudo leer la topología")).is_err());
    }
}
