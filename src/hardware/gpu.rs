use super::MemoryInfo;
use anyhow::{bail, Context, Result};
use std::{
    fs::{self, File},
    io,
    os::fd::AsRawFd,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq)]
pub enum GpuMemoryKind {
    Dedicated,
    Shared,
    ReservedShared,
}

pub struct GpuMemory {
    pub kind: GpuMemoryKind,
    pub available: Option<u64>,
    pub total: u64,
}

pub struct GpuInfo {
    pub name: String,
    pub memory: Result<Vec<GpuMemory>, String>,
}

pub(super) fn read_gpus(ram: Option<&MemoryInfo>) -> Result<Vec<GpuInfo>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir("/sys/bus/pci/devices").context("No se pudo consultar las GPU PCI")? {
        let entry = entry?;
        let class = read_hex(&entry.path().join("class"))?;
        if class >> 16 == 0x03 {
            devices.push(entry.path());
        }
    }
    devices.sort();
    let ids = read_pci_ids()?;
    let mut gpus = Vec::new();
    for device in devices {
        let vendor = read_hex(&device.join("vendor"))?;
        let id = read_hex(&device.join("device"))?;
        let slot = device
            .file_name()
            .context("Dirección PCI no válida")?
            .to_string_lossy();
        let model = ids.as_ref().and_then(|ids| pci_name(ids, vendor, id));
        let name = match model {
            Some(model) => format!("{model} [{slot}]"),
            None => format!("PCI {vendor:04x}:{id:04x} [{slot}]"),
        };
        let memory = (|| {
            let driver =
                fs::read_link(device.join("driver")).context("Sin controlador gráfico activo")?;
            match driver.file_name().and_then(|name| name.to_str()) {
                Some("amdgpu") => amd_memory(&device, ram),
                Some("nvidia") => nvidia_memory(&slot),
                Some("i915") => intel_memory(&device, ram),
                Some(driver) => bail!("Memoria/tipo no consultables con el controlador {driver}"),
                None => bail!("Nombre de controlador no válido"),
            }
        })()
        .map_err(|error| format!("{error:#}"));
        gpus.push(GpuInfo { name, memory });
    }
    Ok(gpus)
}

fn read_hex(path: &Path) -> Result<u32> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    u32::from_str_radix(raw.trim().trim_start_matches("0x"), 16)
        .with_context(|| format!("Identificador no válido en {}", path.display()))
}

fn read_pci_ids() -> Result<Option<String>> {
    for path in ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"] {
        match fs::read_to_string(path) {
            Ok(ids) => return Ok(Some(ids)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).with_context(|| format!("No se pudo leer {path}")),
        }
    }
    Ok(None)
}

fn pci_name(ids: &str, vendor: u32, device: u32) -> Option<String> {
    let mut vendor_name = None;
    for line in ids.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            let (id, name) = line.split_once(' ')?;
            if u32::from_str_radix(id, 16).ok() == Some(vendor) {
                vendor_name = Some(match vendor {
                    0x1002 => "AMD",
                    0x10de => "NVIDIA",
                    0x8086 => "Intel",
                    _ => name.trim(),
                });
            } else if vendor_name.is_some() {
                break;
            }
        } else if !line.starts_with("\t\t") && vendor_name.is_some() {
            let (id, name) = line.trim_start().split_once(' ')?;
            if u32::from_str_radix(id, 16).ok() == Some(device) {
                return Some(format!("{} {}", vendor_name?, name.trim()));
            }
        }
    }
    vendor_name.map(str::to_string)
}

fn render_node(device: &Path) -> Result<File> {
    for entry in fs::read_dir(device.join("drm")).context("Sin nodos DRM para la GPU")? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("renderD") {
            let path = Path::new("/dev/dri").join(entry.file_name());
            return File::options()
                .read(true)
                .write(true)
                .open(&path)
                .with_context(|| format!("No se pudo abrir {}", path.display()));
        }
    }
    bail!("Sin nodo de renderizado para consultar la memoria/tipo")
}

fn ioctl<T>(file: &File, request: libc::Ioctl, data: &mut T) -> Result<()> {
    // All callers use the Linux DRM UAPI layouts and keep nested buffers alive.
    let result = unsafe { libc::ioctl(file.as_raw_fd(), request, data as *mut T) };
    if result < 0 {
        return Err(io::Error::last_os_error()).context("Consulta DRM fallida");
    }
    Ok(())
}

#[repr(C)]
struct AmdQuery {
    pointer: u64,
    size: u32,
    query: u32,
    parameters: [u32; 4],
}

fn amd_integrated(file: &File) -> Result<bool> {
    // AMDGPU_INFO_DEV_INFO: ids_flags is at byte 136 of the stable UAPI prefix.
    let mut info = [0u64; 18];
    let mut query = AmdQuery {
        pointer: info.as_mut_ptr() as u64,
        size: std::mem::size_of_val(&info) as u32,
        query: 0x16,
        parameters: [0; 4],
    };
    ioctl(file, libc::_IOW::<AmdQuery>(b'd' as u32, 0x45), &mut query)?;
    Ok(info[17] & 1 != 0) // AMDGPU_IDS_FLAGS_FUSION
}

fn pool(device: &Path, prefix: &str, kind: GpuMemoryKind) -> Result<GpuMemory> {
    let read = |suffix| -> Result<u64> {
        let path = device.join(format!("mem_info_{prefix}_{suffix}"));
        fs::read_to_string(&path)
            .with_context(|| format!("No se pudo leer {}", path.display()))?
            .trim()
            .parse()
            .context("Valor de memoria GPU no válido")
    };
    let total = read("total")?;
    let used = read("used")?;
    if used > total {
        bail!("Memoria GPU usada/total incoherente");
    }
    Ok(GpuMemory {
        kind,
        available: Some(total - used),
        total,
    })
}

fn amd_memory(device: &Path, ram: Option<&MemoryInfo>) -> Result<Vec<GpuMemory>> {
    let integrated = amd_integrated(&render_node(device)?)?;
    let vram = pool(
        device,
        "vram",
        if integrated {
            GpuMemoryKind::ReservedShared
        } else {
            GpuMemoryKind::Dedicated
        },
    )?;
    let mut shared = pool(device, "gtt", GpuMemoryKind::Shared)?;
    shared.available = shared_available(shared.available, ram);
    Ok(vec![vram, shared])
}

fn shared_available(pool_free: Option<u64>, ram: Option<&MemoryInfo>) -> Option<u64> {
    ram.and_then(|ram| pool_free.map(|free| free.min(ram.available)))
}

fn command_output(command: &mut Command) -> Result<String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("nvidia-smi no está disponible")?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().context("No se pudo detener nvidia-smi")?;
            child.wait()?;
            bail!("nvidia-smi excedió el tiempo de espera (1 s)");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "nvidia-smi ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("Respuesta de nvidia-smi no válida")
}

fn nvidia_memory(slot: &str) -> Result<Vec<GpuMemory>> {
    let raw = command_output(Command::new("nvidia-smi").args([
        "-i",
        slot,
        "--query-gpu=memory.free,memory.total",
        "--format=csv,noheader,nounits",
    ]))?;
    Ok(vec![parse_nvidia_memory(&raw)?])
}

fn parse_nvidia_memory(raw: &str) -> Result<GpuMemory> {
    let (free, total) = raw
        .trim()
        .split_once(',')
        .context("Respuesta de memoria NVIDIA no válida")?;
    let mib = |value: &str| -> Result<u64> {
        value
            .trim()
            .parse::<u64>()
            .context("Memoria NVIDIA no consultable")?
            .checked_mul(1 << 20)
            .context("Memoria NVIDIA fuera de rango")
    };
    let available = mib(free)?;
    let total = mib(total)?;
    if total == 0 || available > total {
        bail!("Memoria NVIDIA disponible/total incoherente");
    }
    Ok(GpuMemory {
        kind: GpuMemoryKind::Dedicated,
        available: Some(available),
        total,
    })
}

#[repr(C)]
struct IntelQueryItem {
    id: u64,
    length: i32,
    flags: u32,
    pointer: u64,
}

#[repr(C)]
struct IntelQuery {
    count: u32,
    flags: u32,
    items: u64,
}

fn intel_memory(device: &Path, ram: Option<&MemoryInfo>) -> Result<Vec<GpuMemory>> {
    let file = render_node(device)?;
    let mut item = IntelQueryItem {
        id: 4,
        length: 0,
        flags: 0,
        pointer: 0,
    };
    let mut query = IntelQuery {
        count: 1,
        flags: 0,
        items: &mut item as *mut _ as u64,
    };
    let request = libc::_IOWR::<IntelQuery>(b'd' as u32, 0x79);
    ioctl(&file, request, &mut query)?;
    if !(16..=65536).contains(&item.length) {
        bail!("El controlador i915 no permite consultar las regiones de memoria");
    }
    let mut data = vec![0u8; item.length as usize];
    item.pointer = data.as_mut_ptr() as u64;
    ioctl(&file, request, &mut query)?;
    if item.length < 0 {
        bail!("Consulta de memoria i915 fallida: {}", item.length);
    }
    parse_intel_memory(&data, ram)
}

fn parse_intel_memory(data: &[u8], ram: Option<&MemoryInfo>) -> Result<Vec<GpuMemory>> {
    let count = u32::from_ne_bytes(
        data.get(..4)
            .context("Respuesta i915 truncada")?
            .try_into()?,
    );
    let mut pools = Vec::new();
    // drm_i915_query_memory_regions has a 16-byte header and 88-byte records.
    for i in 0..count as usize {
        let offset = 16 + i * 88;
        let region = data
            .get(offset..offset + 88)
            .context("Región i915 truncada")?;
        let class = u16::from_ne_bytes(region[..2].try_into()?);
        let total = u64::from_ne_bytes(region[8..16].try_into()?);
        if total == 0 {
            bail!("Región de memoria i915 vacía");
        }
        match class {
            0 => pools.push(GpuMemory {
                kind: GpuMemoryKind::Shared,
                available: ram.map(|ram| ram.available.min(total)),
                total,
            }),
            // i915 does not expose reliable free VRAM without elevated capabilities.
            1 => pools.push(GpuMemory {
                kind: GpuMemoryKind::Dedicated,
                available: None,
                total,
            }),
            _ => bail!("Clase de memoria i915 no compatible: {class}"),
        }
    }
    if pools.is_empty() {
        bail!("Sin regiones de memoria i915");
    }
    Ok(pools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drm_query_layouts_match_linux_uapi() {
        assert_eq!(std::mem::size_of::<AmdQuery>(), 32);
        assert_eq!(std::mem::size_of::<IntelQuery>(), 16);
        assert_eq!(std::mem::size_of::<IntelQueryItem>(), 24);
    }

    #[test]
    fn shared_memory_is_limited_by_both_gpu_pool_and_available_ram() {
        let ram = MemoryInfo {
            available: 2 << 30,
            total: 16 << 30,
        };
        assert_eq!(shared_available(Some(4 << 30), Some(&ram)), Some(2 << 30));
        assert_eq!(shared_available(Some(1 << 30), Some(&ram)), Some(1 << 30));
        assert_eq!(shared_available(Some(0), Some(&ram)), Some(0));
        assert_eq!(shared_available(Some(4 << 30), None), None);
        assert_eq!(shared_available(None, Some(&ram)), None);
    }

    #[test]
    fn pci_names_distinguish_devices_and_vendors() {
        let ids = "# test\n1002  AMD\n\t15e7  Barcelo\n\t\t17aa 50c1  Lenovo\n8086  Intel\n\t15e7  Different\n";
        assert_eq!(
            pci_name(ids, 0x1002, 0x15e7).as_deref(),
            Some("AMD Barcelo")
        );
        assert_eq!(
            pci_name(ids, 0x8086, 0x15e7).as_deref(),
            Some("Intel Different")
        );
        assert_eq!(pci_name(ids, 0x1002, 0xffff).as_deref(), Some("AMD"));
        assert!(pci_name(ids, 0xffff, 0x15e7).is_none());
    }

    #[test]
    fn nvidia_values_are_converted_and_validated() {
        let memory = parse_nvidia_memory("4096, 8192\n").unwrap();
        assert_eq!(memory.kind, GpuMemoryKind::Dedicated);
        assert_eq!(memory.available, Some(4 << 30));
        assert_eq!(memory.total, 8 << 30);
        for raw in ["N/A, 8192", "9, 8", "0, 0", "bad", "1, 2, 3"] {
            assert!(parse_nvidia_memory(raw).is_err());
        }
    }

    #[test]
    fn intel_shared_uses_ram_and_dedicated_does_not_invent_free_memory() {
        let mut data = vec![0; 16 + 2 * 88];
        data[..4].copy_from_slice(&2u32.to_ne_bytes());
        data[24..32].copy_from_slice(&(8u64 << 30).to_ne_bytes());
        data[104..106].copy_from_slice(&1u16.to_ne_bytes());
        data[112..120].copy_from_slice(&(4u64 << 30).to_ne_bytes());
        let ram = MemoryInfo {
            available: 2 << 30,
            total: 16 << 30,
        };
        let pools = parse_intel_memory(&data, Some(&ram)).unwrap();
        assert_eq!(pools[0].kind, GpuMemoryKind::Shared);
        assert_eq!(pools[0].available, Some(2 << 30));
        assert_eq!(pools[1].kind, GpuMemoryKind::Dedicated);
        assert_eq!(pools[1].available, None);
        assert_eq!(parse_intel_memory(&data, None).unwrap()[0].available, None);
        assert!(parse_intel_memory(&data[..100], Some(&ram)).is_err());
    }
}
