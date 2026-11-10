#!/usr/bin/env bun
import { spawnSync } from "child_process";
import { existsSync, readFileSync } from "fs";
import { join } from "path";

console.log("====================================================================");
console.log("Sovereign Hardware & Compiler Benchmark Suite (Bun Entrypoint)");
console.log("Intel i7-13700F 16C/24T | 32GB DDR5-5800 | RTX 4070 sm_89 CUDA 13.4");
console.log("Runtime: Bun v" + Bun.version + " (Full-Rust Engine + JavaScriptCore C++)");
console.log("====================================================================");

const stackBenchExe = join(import.meta.dir, "target", "release", "stack-bench.exe");

// Ensure Rust sovereign binary is built
if (!existsSync(stackBenchExe)) {
    console.log("\nBuilding stack-bench.exe with fat LTO, codegen-units=1, panic=abort...");
    const buildRes = spawnSync("cargo", ["build", "--release", "--target-dir", join(import.meta.dir, "target")], {
        stdio: "inherit",
        cwd: import.meta.dir,
    });
    if (buildRes.status !== 0) {
        console.error("Cargo release build failed");
        process.exit(1);
    }
}

// 1. Run Tier 5 Bun WASM benchmark
console.log("\n[Tier 5] Executing Bun + WASM Kernel...");
const bunBench = spawnSync("bun", ["run", join(import.meta.dir, "tier5_bun", "bench.ts")], {
    stdio: "inherit",
    cwd: import.meta.dir,
});

// 2. Run full report generation via stack-bench.exe
console.log("\n[All Tiers] Executing stack-bench.exe report...");
const reportRes = spawnSync(stackBenchExe, ["report"], {
    stdio: "inherit",
    cwd: import.meta.dir,
});

// 3. Demonstrate Deterministic Stack Picker for all 5 sovereign workloads
const workloads = ["gpu-compute", "compositor", "systems-cli", "web-edge", "wasm-app"];
console.log("\n====================================================================");
console.log("Running Deterministic Stack Picker across Sovereign Workloads");
console.log("====================================================================");

for (const w of workloads) {
    console.log(`\n>>> Evaluating Workload: ${w}`);
    spawnSync(stackBenchExe, ["pick", "--workload", w], {
        stdio: "inherit",
        cwd: import.meta.dir,
    });
}

console.log("\nAll benchmarks complete. Report verified at BENCHMARK_REPORT.md");
