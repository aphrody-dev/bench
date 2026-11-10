pub mod cold_start;
pub mod hardware;
pub mod picker;
pub mod public_bench;
pub mod tier1_cuda;
pub mod tier2_rust;
pub mod tier3_msvc;
pub mod tier4_dotnet;
pub mod tier5_bun;
pub mod zerocopy_bench;

#[cfg(test)]
mod tests {
    use super::*;
    use picker::{pick_stack, Workload};

    #[test]
    fn test_picker_decisions() {
        let workloads = [
            (Workload::GpuCompute, "Tier 1 — CUDA 13.4 Native"),
            (Workload::Compositor, "Tier 2 — Rust Sovereign AOT"),
            (Workload::SystemsCli, "Tier 2 — Rust Sovereign AOT"),
            (Workload::WebEdge, "Tier 5 — Pure oven-sh/bun"),
            (Workload::WasmApp, "Tier 5 — Pure oven-sh/bun + Inline WebAssembly AOT"),
        ];

        for (w, expected_tier_prefix) in workloads {
            let d = pick_stack(w, 23967.0, 920.0, 466.5, 65.0, 2500.0, 15000.0);
            assert!(
                d.winning_tier.starts_with(expected_tier_prefix),
                "Workload {:?} expected tier prefix '{}', got '{}'",
                w,
                expected_tier_prefix,
                d.winning_tier
            );
            assert!(!d.lto_strategy.is_empty());
            assert!(!d.aot_strategy.is_empty());
            assert!(!d.zero_copy_mechanism.is_empty());
        }
    }

    #[test]
    fn test_workload_parsing() {
        assert_eq!(Workload::parse("gpu-compute"), Some(Workload::GpuCompute));
        assert_eq!(Workload::parse("compositor"), Some(Workload::Compositor));
        assert_eq!(Workload::parse("systems-cli"), Some(Workload::SystemsCli));
        assert_eq!(Workload::parse("web-edge"), Some(Workload::WebEdge));
        assert_eq!(Workload::parse("wasm-app"), Some(Workload::WasmApp));
        assert_eq!(Workload::parse("invalid"), None);
    }

    #[test]
    fn test_hardware_specs_detect() {
        let specs = hardware::HardwareSpecs::detect();
        assert!(specs.cpu.contains("i7-13700F"));
        assert!(specs.gpu.contains("RTX 4070"));
        assert_eq!(specs.gpu_architecture, "Ada Lovelace (sm_89)");
    }

    #[test]
    fn test_mandelbrot_correctness() {
        let sum = public_bench::run_rust_mandelbrot(32);
        assert!(sum > 0);
    }

    #[test]
    fn test_qpc_timer() {
        let t = hardware::QpcInstant::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(t.elapsed_nanos() > 0.0);
        assert!(t.elapsed_micros() > 0.0);
        assert!(t.elapsed_secs() > 0.0);
    }
}
