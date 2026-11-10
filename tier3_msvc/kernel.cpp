#include <immintrin.h>
#include <windows.h>
#include <iostream>
#include <vector>
#include <thread>
#include <chrono>
#include <string>
#include <cstdio>
#include <numeric>

static inline double get_time_sec() {
    LARGE_INTEGER count, freq;
    QueryPerformanceCounter(&count);
    QueryPerformanceFrequency(&freq);
    return (double)count.QuadPart / (double)freq.QuadPart;
}

static inline uint64_t get_time_ns() {
    LARGE_INTEGER count, freq;
    QueryPerformanceCounter(&count);
    QueryPerformanceFrequency(&freq);
    return (uint64_t)(((double)count.QuadPart * 1e9) / (double)freq.QuadPart);
}

void fma_compute_worker(float* data, size_t count, int iters, float alpha, float beta) {
    size_t vec_count = count / 8;
    __m256 a = _mm256_set1_ps(alpha);
    __m256 b = _mm256_set1_ps(beta);

    for (size_t i = 0; i < vec_count; ++i) {
        __m256 v0 = _mm256_loadu_ps(data + i * 8);
        __m256 v1 = _mm256_add_ps(v0, _mm256_set1_ps(0.1f));
        __m256 v2 = _mm256_add_ps(v0, _mm256_set1_ps(0.2f));
        __m256 v3 = _mm256_add_ps(v0, _mm256_set1_ps(0.3f));

        #pragma unroll 8
        for (int it = 0; it < iters; ++it) {
            v0 = _mm256_fmadd_ps(v0, a, b);
            v1 = _mm256_fmadd_ps(v1, a, b);
            v2 = _mm256_fmadd_ps(v2, a, b);
            v3 = _mm256_fmadd_ps(v3, a, b);
        }

        __m256 sum = _mm256_add_ps(_mm256_add_ps(v0, v1), _mm256_add_ps(v2, v3));
        _mm256_storeu_ps(data + i * 8, sum);
    }
}

void triad_worker(const float* a, const float* b, float* c, size_t count, float alpha) {
    size_t vec_count = count / 8;
    __m256 valpha = _mm256_set1_ps(alpha);

    for (size_t i = 0; i < vec_count; ++i) {
        __m256 va = _mm256_loadu_ps(a + i * 8);
        __m256 vb = _mm256_loadu_ps(b + i * 8);
        __m256 vc = _mm256_fmadd_ps(vb, valpha, va);
        _mm256_storeu_ps(c + i * 8, vc);
    }
}

int main(int argc, char** argv) {
    bool json_output = false;
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--json") {
            json_output = true;
        }
    }

    const unsigned int num_threads = std::thread::hardware_concurrency();

    // Compute workload
    const size_t compute_n = 4194304; // 4M floats
    const int fma_iters = 64;         // 4 vec * 8 floats * 64 iters * 2 flops = 4096 flops per 8 elements = 512 flops/element
    const double total_compute_flops = (double)compute_n * 4.0 * fma_iters * 2.0;

    std::vector<float> compute_buf(compute_n, 1.0f);

    // Streaming workload: 33.5M floats per array = 134 MB -> 3 arrays = 402 MB (exceeds i7-13700F 30MB L3 cache)
    const size_t stream_n = 33554432;
    const size_t stream_bytes = stream_n * sizeof(float);
    const double total_stream_bytes = (double)stream_bytes * 3.0; // 2 reads + 1 write

    std::vector<float> a(stream_n, 1.0f);
    std::vector<float> b(stream_n, 2.0f);
    std::vector<float> c(stream_n, 0.0f);

    // Compute Benchmark (Multi-threaded AVX2 FMA)
    double min_compute_time = 1e9;
    for (int run = 0; run < 5; ++run) {
        double t0 = get_time_sec();
        std::vector<std::thread> workers;
        size_t chunk = compute_n / num_threads;
        for (unsigned int t = 0; t < num_threads; ++t) {
            size_t offset = t * chunk;
            size_t len = (t == num_threads - 1) ? (compute_n - offset) : chunk;
            workers.emplace_back(fma_compute_worker, compute_buf.data() + offset, len, fma_iters, 1.0001f, 0.0001f);
        }
        for (auto& w : workers) w.join();
        double t1 = get_time_sec();
        double dt = t1 - t0;
        if (dt < min_compute_time) min_compute_time = dt;
    }

    double compute_gflops = (total_compute_flops / min_compute_time) / 1e9;
    double compute_latency_ns = min_compute_time * 1e9;

    // Stream Triad Benchmark (Multi-threaded AVX2 Memory Streaming)
    double min_stream_time = 1e9;
    for (int run = 0; run < 5; ++run) {
        double t0 = get_time_sec();
        std::vector<std::thread> workers;
        size_t chunk = stream_n / num_threads;
        for (unsigned int t = 0; t < num_threads; ++t) {
            size_t offset = t * chunk;
            size_t len = (t == num_threads - 1) ? (stream_n - offset) : chunk;
            workers.emplace_back(triad_worker, a.data() + offset, b.data() + offset, c.data() + offset, len, 3.0f);
        }
        for (auto& w : workers) w.join();
        double t1 = get_time_sec();
        double dt = t1 - t0;
        if (dt < min_stream_time) min_stream_time = dt;
    }

    double stream_bandwidth_gbps = (total_stream_bytes / min_stream_time) / 1e9;
    double stream_latency_ns = min_stream_time * 1e9;

    if (json_output) {
        printf("{\n");
        printf("  \"tier\": \"Tier 3 - MSVC 14.51 C++23 AOT (/O2 /GL /LTCG /arch:AVX2)\",\n");
        printf("  \"threads\": %u,\n", num_threads);
        printf("  \"fma_gflops\": %.2f,\n", compute_gflops);
        printf("  \"fma_latency_ns\": %.2f,\n", compute_latency_ns);
        printf("  \"stream_bandwidth_gbps\": %.2f,\n", stream_bandwidth_gbps);
        printf("  \"stream_latency_ns\": %.2f,\n", stream_latency_ns);
        printf("  \"compiler_flags\": \"/O2 /GL /LTCG /arch:AVX2 /std:c++latest /fp:fast\"\n");
        printf("}\n");
    } else {
        printf("====================================================================\n");
        printf("Tier 3 - MSVC 14.51 C++23 AOT Benchmark Result\n");
        printf("Threads: %u (Intel i7-13700F 16C/24T)\n", num_threads);
        printf("FP32 FMA Compute: %.2f GFLOPS (Latency: %.2f us)\n", compute_gflops, compute_latency_ns / 1000.0);
        printf("DDR5-5800 Triad Bandwidth: %.2f GB/s (Latency: %.2f us)\n", stream_bandwidth_gbps, stream_latency_ns / 1000.0);
        printf("====================================================================\n");
    }

    return 0;
}
