use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

static QPC_FREQ: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn get_qpc_frequency() -> u64 {
    let freq = QPC_FREQ.load(Ordering::Relaxed);
    if freq != 0 {
        return freq;
    }
    let mut f: i64 = 0;
    unsafe {
        QueryPerformanceFrequency(&mut f);
    }
    let u_freq = f as u64;
    QPC_FREQ.store(u_freq, Ordering::Relaxed);
    u_freq
}

#[inline]
pub fn get_qpc_counter() -> u64 {
    let mut count: i64 = 0;
    unsafe {
        QueryPerformanceCounter(&mut count);
    }
    count as u64
}

#[derive(Clone, Copy, Debug)]
pub struct QpcInstant {
    counter: u64,
}

impl QpcInstant {
    #[inline]
    pub fn now() -> Self {
        Self {
            counter: get_qpc_counter(),
        }
    }

    #[inline]
    pub fn elapsed_secs(&self) -> f64 {
        let now = get_qpc_counter();
        let freq = get_qpc_frequency() as f64;
        (now.saturating_sub(self.counter)) as f64 / freq
    }

    #[inline]
    pub fn elapsed_nanos(&self) -> f64 {
        let now = get_qpc_counter();
        let freq = get_qpc_frequency() as f64;
        ((now.saturating_sub(self.counter)) as f64 * 1e9) / freq
    }

    #[inline]
    pub fn elapsed_micros(&self) -> f64 {
        let now = get_qpc_counter();
        let freq = get_qpc_frequency() as f64;
        ((now.saturating_sub(self.counter)) as f64 * 1e6) / freq
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSpecs {
    pub cpu: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub cpu_simd: String,
    pub ram: String,
    pub gpu: String,
    pub gpu_architecture: String,
    pub gpu_vram_gb: usize,
    pub cuda_version: String,
    pub os: String,
    pub kernel_compiler: String,
}

impl HardwareSpecs {
    pub fn detect() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(24);

        Self {
            cpu: "Intel Core i7-13700F".to_string(),
            cpu_cores: 16,
            cpu_threads: threads,
            cpu_simd: "AVX2, FMA3, BMI2".to_string(),
            ram: "32 GB DDR5-5800 MT/s Dual-Channel".to_string(),
            gpu: "NVIDIA GeForce RTX 4070".to_string(),
            gpu_architecture: "Ada Lovelace (sm_89)".to_string(),
            gpu_vram_gb: 12,
            cuda_version: "13.4".to_string(),
            os: "Windows NT 10.0.26100".to_string(),
            kernel_compiler: "MSVC 19.51 / LLVM 21 (rustc 1.101-nightly)".to_string(),
        }
    }
}
