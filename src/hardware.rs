mod cpu;
mod gpu;
mod memory;

pub use cpu::CpuInfo;
#[cfg(test)]
pub use gpu::GpuMemory;
pub use gpu::{GpuInfo, GpuMemoryKind};
pub use memory::MemoryInfo;

pub struct HardwareInfo {
    pub memory: Result<MemoryInfo, String>,
    pub cpu: Result<CpuInfo, String>,
    pub gpus: Result<Vec<GpuInfo>, String>,
}

impl HardwareInfo {
    pub fn read() -> Self {
        let memory = memory::read_memory().map_err(|error| format!("{error:#}"));
        let gpus = gpu::read_gpus(memory.as_ref().ok()).map_err(|error| format!("{error:#}"));
        Self {
            memory,
            cpu: cpu::read_cpu().map_err(|error| format!("{error:#}")),
            gpus,
        }
    }
}
