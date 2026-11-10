use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tier4Metrics {
    pub tier: String,
    pub runtime_version: String,
    pub threads: u32,
    pub fma_gflops: f64,
    pub fma_latency_ns: f64,
    pub stream_bandwidth_gbps: f64,
    pub stream_latency_ns: f64,
    pub zero_copy_latency_ns: f64,
    pub zero_copy_mechanism: String,
}

pub fn run_benchmark() -> Tier4Metrics {
    let aot_exe = Path::new("tier4_dotnet/publish/tier4_dotnet.exe");
    let output = if aot_exe.exists() {
        Command::new(aot_exe)
            .arg("--json")
            .output()
            .expect("Failed to execute tier4_dotnet.exe")
    } else {
        println!("[tier4_dotnet] Running via dotnet run -c Release...");
        Command::new("dotnet")
            .args(["run", "--project", "tier4_dotnet", "-c", "Release", "--", "--json"])
            .output()
            .expect("Failed to execute dotnet run")
    };

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        panic!("tier4_dotnet failed: {}", err);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract JSON block if there are other output lines
    let json_start = stdout.find('{').unwrap_or(0);
    let json_end = stdout.rfind('}').map(|idx| idx + 1).unwrap_or(stdout.len());
    let json_str = &stdout[json_start..json_end];

    serde_json::from_str::<Tier4Metrics>(json_str)
        .unwrap_or_else(|e| panic!("Failed to parse Tier 4 JSON output: {}. Output was: {}", e, stdout))
}
