use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tier3Metrics {
    pub tier: String,
    pub threads: u32,
    pub fma_gflops: f64,
    pub fma_latency_ns: f64,
    pub stream_bandwidth_gbps: f64,
    pub stream_latency_ns: f64,
    pub compiler_flags: String,
}

pub fn run_benchmark() -> Tier3Metrics {
    let exe_path = Path::new("tier3_msvc/kernel.exe");
    if !exe_path.exists() {
        println!("[tier3_msvc] Compiling kernel.cpp with MSVC cl.exe /O2 /GL /LTCG /arch:AVX2...");
        let build_cmd = r#"call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat" && cl /O2 /GL /arch:AVX2 /std:c++latest /fp:fast /EHsc /Fotier3_msvc/kernel.obj /Fetier3_msvc/kernel.exe tier3_msvc/kernel.cpp /link /LTCG"#;
        let status = Command::new("cmd")
            .args(["/c", build_cmd])
            .status()
            .expect("Failed to execute MSVC cl build");
        if !status.success() {
            panic!("MSVC compilation failed");
        }
    }

    let output = Command::new("tier3_msvc/kernel.exe")
        .arg("--json")
        .output()
        .expect("Failed to execute tier3_msvc/kernel.exe");

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        panic!("tier3_msvc failed: {}", err);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<Tier3Metrics>(&stdout)
        .unwrap_or_else(|e| panic!("Failed to parse Tier 3 JSON output: {}. Output was: {}", e, stdout))
}
