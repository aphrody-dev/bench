use crate::hardware::QpcInstant;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryProfile {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub size_mb: f64,
    pub cold_start_us: f64,
    pub build_tier: String,
    pub lto_aot_config: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColdStartReport {
    pub profiles: Vec<BinaryProfile>,
    pub smallest_binary: String,
    pub fastest_cold_start: String,
}

fn measure_binary(name: &str, path: &str, args: &[&str], tier: &str, config: &str) -> Option<BinaryProfile> {
    let p = Path::new(path);
    if !p.exists() {
        return None;
    }

    let size_bytes = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    let mut min_us = f64::MAX;
    for _ in 0..5 {
        let t0 = QpcInstant::now();
        let _ = Command::new(path).args(args).output();
        let dt = t0.elapsed_micros();
        if dt < min_us {
            min_us = dt;
        }
    }

    Some(BinaryProfile {
        name: name.to_string(),
        path: path.to_string(),
        size_bytes,
        size_mb,
        cold_start_us: min_us,
        build_tier: tier.to_string(),
        lto_aot_config: config.to_string(),
    })
}

pub fn run_cold_start_benchmarks() -> ColdStartReport {
    let mut profiles = Vec::new();

    // 1. Rust Sovereign AOT + Fat LTO + Strip
    if let Some(p) = measure_binary(
        "Rust Sovereign AOT",
        "target/release/stack-bench.exe",
        &["--version"],
        "Tier 2 - Rust Sovereign",
        "lto=\"fat\", codegen-units=1, panic=\"abort\", strip=true, -C target-cpu=native",
    ) {
        profiles.push(p);
    }

    // 2. MSVC C++23 AOT + LTCG
    if let Some(p) = measure_binary(
        "MSVC C++23 AOT",
        "tier3_msvc/kernel.exe",
        &["--json"],
        "Tier 3 - MSVC C++23",
        "/O2 /GL /LTCG /arch:AVX2 /fp:fast",
    ) {
        profiles.push(p);
    }

    // 3. .NET 11 NativeAOT
    if let Some(p) = measure_binary(
        ".NET 11 NativeAOT",
        "tier4_dotnet/publish/tier4_dotnet.exe",
        &["--json"],
        "Tier 4 - .NET 11 NativeAOT",
        "PublishAot=true, TieredPGO=true, OptimizationPreference=Speed",
    ) {
        profiles.push(p);
    }

    // 4. CUDA 13.4 sm_89
    if let Some(p) = measure_binary(
        "CUDA 13.4 Native",
        "tier1_cuda/kernel.exe",
        &["--json"],
        "Tier 1 - CUDA 13.4 Native",
        "nvcc -O3 -arch=sm_89 --use_fast_math",
    ) {
        profiles.push(p);
    }

    // Sort by cold start
    let mut sorted_by_cold = profiles.clone();
    sorted_by_cold.sort_by(|a, b| a.cold_start_us.partial_cmp(&b.cold_start_us).unwrap());

    // Sort by size
    let mut sorted_by_size = profiles.clone();
    sorted_by_size.sort_by(|a, b| a.size_bytes.cmp(&b.size_bytes));

    let smallest = sorted_by_size.first().map(|p| p.name.clone()).unwrap_or_default();
    let fastest = sorted_by_cold.first().map(|p| p.name.clone()).unwrap_or_default();

    ColdStartReport {
        profiles,
        smallest_binary: smallest,
        fastest_cold_start: fastest,
    }
}
