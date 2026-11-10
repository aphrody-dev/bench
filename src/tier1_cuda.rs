use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tier1Metrics {
    pub tier: String,
    pub device: String,
    pub sm_count: u32,
    pub compute_capability: String,
    pub fma_gflops: f64,
    pub fma_latency_ns: f64,
    pub stream_bandwidth_gbps: f64,
    pub stream_latency_ns: f64,
    pub zero_copy_mechanism: String,
}

pub fn run_benchmark() -> Tier1Metrics {
    let exe_path = Path::new("tier1_cuda/kernel.exe");
    if !exe_path.exists() {
        println!("[tier1_cuda] Compiling kernel.cu with nvcc -O3 -arch=sm_89 --use_fast_math...");
        let status = Command::new("nvcc")
            .args(["-O3", "-arch=sm_89", "--use_fast_math", "-o", "tier1_cuda/kernel.exe", "tier1_cuda/kernel.cu"])
            .status()
            .expect("Failed to execute nvcc");
        if !status.success() {
            panic!("nvcc compilation failed");
        }
    }

    let output = Command::new("tier1_cuda/kernel.exe")
        .arg("--json")
        .output()
        .expect("Failed to execute tier1_cuda/kernel.exe");

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        panic!("tier1_cuda failed: {}", err);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<Tier1Metrics>(&stdout)
        .unwrap_or_else(|e| panic!("Failed to parse Tier 1 JSON output: {}. Output was: {}", e, stdout))
}
