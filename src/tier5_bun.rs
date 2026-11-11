use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tier5Metrics {
    pub tier: String,
    pub runtime: String,
    pub engine: String,
    pub fma_gflops: f64,
    pub fma_latency_ns: f64,
    pub stream_bandwidth_gbps: f64,
    pub stream_latency_ns: f64,
    pub zero_copy_latency_ns: f64,
    pub heap_clone_latency_ns: f64,
    pub zero_copy_speedup: f64,
    pub octane_mandelbrot_latency_us: f64,
    pub shell_spawn_us: f64,
    pub zero_copy_mechanism: String,
}

pub fn run_benchmark() -> Tier5Metrics {
    let script_path = r"C:\src\infra\ffi-bridge\src\tier5-bun-bench.ts";
    let output = Command::new("bun")
        .args(["run", script_path, "--json"])
        .output()
        .expect("Failed to execute bun");

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        panic!("tier5_bun failed: {}", err);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').unwrap_or(0);
    let json_end = stdout.rfind('}').map(|idx| idx + 1).unwrap_or(stdout.len());
    let json_str = &stdout[json_start..json_end];

    serde_json::from_str::<Tier5Metrics>(json_str)
        .unwrap_or_else(|e| panic!("Failed to parse Tier 5 JSON output: {}. Output was: {}", e, stdout))
}
