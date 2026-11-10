use crate::hardware::QpcInstant;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tier2Metrics {
    pub tier: String,
    pub threads: usize,
    pub fma_gflops: f64,
    pub fma_latency_ns: f64,
    pub stream_bandwidth_gbps: f64,
    pub stream_latency_ns: f64,
    pub compiler_flags: String,
    pub lto_strategy: String,
}

#[target_feature(enable = "avx2,fma")]
unsafe fn fma_compute_slice(data: &mut [f32], iters: usize, alpha: f32, beta: f32) {
    let len = data.len();
    let vec_len = len / 8;
    let ptr = data.as_mut_ptr();

    let va = _mm256_set1_ps(alpha);
    let vb = _mm256_set1_ps(beta);
    let vc1 = _mm256_set1_ps(0.1);
    let vc2 = _mm256_set1_ps(0.2);
    let vc3 = _mm256_set1_ps(0.3);

    for i in 0..vec_len {
        let p = ptr.add(i * 8);
        let mut v0 = _mm256_loadu_ps(p);
        let mut v1 = _mm256_add_ps(v0, vc1);
        let mut v2 = _mm256_add_ps(v0, vc2);
        let mut v3 = _mm256_add_ps(v0, vc3);

        for _ in 0..iters {
            v0 = _mm256_fmadd_ps(v0, va, vb);
            v1 = _mm256_fmadd_ps(v1, va, vb);
            v2 = _mm256_fmadd_ps(v2, va, vb);
            v3 = _mm256_fmadd_ps(v3, va, vb);
        }

        let sum1 = _mm256_add_ps(v0, v1);
        let sum2 = _mm256_add_ps(v2, v3);
        let sum = _mm256_add_ps(sum1, sum2);
        _mm256_storeu_ps(p, sum);
    }
}

#[target_feature(enable = "avx2,fma")]
unsafe fn triad_slice(a: &[f32], b: &[f32], c: &mut [f32], alpha: f32) {
    let len = c.len();
    let vec_len = len / 8;
    let pa = a.as_ptr();
    let pb = b.as_ptr();
    let pc = c.as_mut_ptr();
    let valpha = _mm256_set1_ps(alpha);

    for i in 0..vec_len {
        let va = _mm256_loadu_ps(pa.add(i * 8));
        let vb = _mm256_loadu_ps(pb.add(i * 8));
        let vc = _mm256_fmadd_ps(vb, valpha, va);
        _mm256_storeu_ps(pc.add(i * 8), vc);
    }
}

pub fn run_benchmark() -> Tier2Metrics {
    let threads = rayon::current_num_threads();

    // 1. Compute Benchmark (4M floats, 64 iters * 4 unrolls * 8 floats * 2 flops = 4096 flops/vec = 512 flops/element)
    const COMPUTE_N: usize = 4_194_304;
    const FMA_ITERS: usize = 64;
    let total_flops = COMPUTE_N as f64 * 4.0 * FMA_ITERS as f64 * 2.0;

    let mut compute_buf = vec![1.0f32; COMPUTE_N];

    // Warmup
    compute_buf.par_chunks_mut(COMPUTE_N / threads).for_each(|chunk| {
        unsafe { fma_compute_slice(chunk, 8, 1.0001, 0.0001); }
    });

    let mut min_compute_secs = f64::MAX;
    let mut min_compute_ns = f64::MAX;

    for _ in 0..5 {
        let t0 = QpcInstant::now();
        let chunk_size = COMPUTE_N / threads;
        compute_buf.par_chunks_mut(chunk_size).for_each(|chunk| {
            unsafe { fma_compute_slice(chunk, FMA_ITERS, 1.0001, 0.0001); }
        });
        let dt_secs = t0.elapsed_secs();
        let dt_ns = t0.elapsed_nanos();
        if dt_secs < min_compute_secs {
            min_compute_secs = dt_secs;
            min_compute_ns = dt_ns;
        }
    }

    let fma_gflops = (total_flops / min_compute_secs) / 1e9;

    // 2. Stream Triad Benchmark (33.5M floats per array = 134 MB each -> 402 MB total traffic)
    const STREAM_N: usize = 33_554_432;
    let stream_bytes = STREAM_N * std::mem::size_of::<f32>();
    let total_stream_bytes = (stream_bytes as f64) * 3.0;

    let a = vec![1.0f32; STREAM_N];
    let b = vec![2.0f32; STREAM_N];
    let mut c = vec![0.0f32; STREAM_N];

    let mut min_stream_secs = f64::MAX;
    let mut min_stream_ns = f64::MAX;

    for _ in 0..5 {
        let t0 = QpcInstant::now();
        let chunk_size = STREAM_N / threads;
        c.par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(i, c_chunk)| {
                let offset = i * chunk_size;
                let a_chunk = &a[offset..offset + c_chunk.len()];
                let b_chunk = &b[offset..offset + c_chunk.len()];
                unsafe { triad_slice(a_chunk, b_chunk, c_chunk, 3.0); }
            });
        let dt_secs = t0.elapsed_secs();
        let dt_ns = t0.elapsed_nanos();
        if dt_secs < min_stream_secs {
            min_stream_secs = dt_secs;
            min_stream_ns = dt_ns;
        }
    }

    let stream_bandwidth_gbps = (total_stream_bytes / min_stream_secs) / 1e9;

    Tier2Metrics {
        tier: "Tier 2 - Rust Sovereign AOT + Fat LTO + zerocopy".to_string(),
        threads,
        fma_gflops,
        fma_latency_ns: min_compute_ns,
        stream_bandwidth_gbps,
        stream_latency_ns: min_stream_ns,
        compiler_flags: "rustc -C opt-level=3 -C lto=fat -C codegen-units=1 -C target-cpu=native -C panic=abort".to_string(),
        lto_strategy: "fat".to_string(),
    }
}
