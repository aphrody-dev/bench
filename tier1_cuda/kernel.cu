#include <cuda_runtime.h>
#include <iostream>
#include <vector>
#include <chrono>
#include <string>
#include <cstdio>

#define CUDA_CHECK(call) \
    do { \
        cudaError_t err = call; \
        if (err != cudaSuccess) { \
            fprintf(stderr, "CUDA error at %s:%d: %s\n", __FILE__, __LINE__, cudaGetErrorString(err)); \
            exit(1); \
        } \
    } while (0)

__global__ void triad_kernel(const float* __restrict__ a, const float* __restrict__ b, float* __restrict__ c, float alpha, int n) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n) {
        c[idx] = a[idx] + alpha * b[idx];
    }
}

__global__ void fma_compute_kernel(float* data, float alpha, float beta, int n, int iters) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n) {
        float v0 = data[idx];
        float v1 = v0 + 0.1f;
        float v2 = v0 + 0.2f;
        float v3 = v0 + 0.3f;
        #pragma unroll 8
        for (int i = 0; i < iters; ++i) {
            v0 = fmaf(v0, alpha, beta);
            v1 = fmaf(v1, alpha, beta);
            v2 = fmaf(v2, alpha, beta);
            v3 = fmaf(v3, alpha, beta);
        }
        data[idx] = v0 + v1 + v2 + v3;
    }
}

int main(int argc, char** argv) {
    bool json_output = false;
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--json") {
            json_output = true;
        }
    }

    int device = 0;
    CUDA_CHECK(cudaSetDevice(device));
    cudaDeviceProp prop;
    CUDA_CHECK(cudaGetDeviceProperties(&prop, device));

    // Problem sizes
    const int compute_n = 4194304; // 4M floats
    const int fma_iters = 256;     // 4 floats * 256 iters * 2 flops = 2048 flops/element
    const double total_compute_flops = (double)compute_n * fma_iters * 4.0 * 2.0;

    const int stream_n = 33554432; // 32M floats = 128 MB per array -> 3 arrays = 384 MB (exceeds 36MB L2)
    const size_t stream_bytes = (size_t)stream_n * sizeof(float);
    const double total_stream_bytes = (double)stream_bytes * 3.0; // 2 reads + 1 write

    // Allocate GPU memory for compute
    float* d_compute = nullptr;
    CUDA_CHECK(cudaMalloc(&d_compute, compute_n * sizeof(float)));
    CUDA_CHECK(cudaMemset(d_compute, 0, compute_n * sizeof(float)));

    // Allocate GPU memory for streaming
    float *d_a = nullptr, *d_b = nullptr, *d_c = nullptr;
    CUDA_CHECK(cudaMalloc(&d_a, stream_bytes));
    CUDA_CHECK(cudaMalloc(&d_b, stream_bytes));
    CUDA_CHECK(cudaMalloc(&d_c, stream_bytes));
    CUDA_CHECK(cudaMemset(d_a, 1, stream_bytes));
    CUDA_CHECK(cudaMemset(d_b, 2, stream_bytes));

    cudaEvent_t start, stop;
    CUDA_CHECK(cudaEventCreate(&start));
    CUDA_CHECK(cudaEventCreate(&stop));

    int blockSize = 256;
    int numBlocksCompute = (compute_n + blockSize - 1) / blockSize;
    int numBlocksStream = (stream_n + blockSize - 1) / blockSize;

    // Warmup
    fma_compute_kernel<<<numBlocksCompute, blockSize>>>(d_compute, 1.0001f, 0.0001f, compute_n, 32);
    triad_kernel<<<numBlocksStream, blockSize>>>(d_a, d_b, d_c, 3.0f, stream_n);
    CUDA_CHECK(cudaDeviceSynchronize());

    // Compute Benchmark (FMA GFLOPS)
    const int compute_runs = 5;
    float min_compute_ms = 1e9f;
    for (int i = 0; i < compute_runs; ++i) {
        CUDA_CHECK(cudaEventRecord(start, 0));
        fma_compute_kernel<<<numBlocksCompute, blockSize>>>(d_compute, 1.0001f, 0.0001f, compute_n, fma_iters);
        CUDA_CHECK(cudaEventRecord(stop, 0));
        CUDA_CHECK(cudaEventSynchronize(stop));

        float ms = 0.0f;
        CUDA_CHECK(cudaEventElapsedTime(&ms, start, stop));
        if (ms < min_compute_ms) min_compute_ms = ms;
    }

    double compute_time_sec = (double)min_compute_ms / 1000.0;
    double compute_gflops = (total_compute_flops / compute_time_sec) / 1e9;
    double compute_latency_ns = (double)min_compute_ms * 1e6;

    // Streaming Benchmark (GDDR6X Bandwidth GB/s)
    const int stream_runs = 5;
    float min_stream_ms = 1e9f;
    for (int i = 0; i < stream_runs; ++i) {
        CUDA_CHECK(cudaEventRecord(start, 0));
        triad_kernel<<<numBlocksStream, blockSize>>>(d_a, d_b, d_c, 3.0f, stream_n);
        CUDA_CHECK(cudaEventRecord(stop, 0));
        CUDA_CHECK(cudaEventSynchronize(stop));

        float ms = 0.0f;
        CUDA_CHECK(cudaEventElapsedTime(&ms, start, stop));
        if (ms < min_stream_ms) min_stream_ms = ms;
    }

    double stream_time_sec = (double)min_stream_ms / 1000.0;
    double stream_bandwidth_gbps = (total_stream_bytes / stream_time_sec) / 1e9;
    double stream_latency_ns = (double)min_stream_ms * 1e6;

    CUDA_CHECK(cudaFree(d_compute));
    CUDA_CHECK(cudaFree(d_a));
    CUDA_CHECK(cudaFree(d_b));
    CUDA_CHECK(cudaFree(d_c));
    CUDA_CHECK(cudaEventDestroy(start));
    CUDA_CHECK(cudaEventDestroy(stop));

    if (json_output) {
        printf("{\n");
        printf("  \"tier\": \"Tier 1 - CUDA 13.4 Native (sm_89)\",\n");
        printf("  \"device\": \"%s\",\n", prop.name);
        printf("  \"sm_count\": %d,\n", prop.multiProcessorCount);
        printf("  \"compute_capability\": \"%d.%d\",\n", prop.major, prop.minor);
        printf("  \"fma_gflops\": %.2f,\n", compute_gflops);
        printf("  \"fma_latency_ns\": %.2f,\n", compute_latency_ns);
        printf("  \"stream_bandwidth_gbps\": %.2f,\n", stream_bandwidth_gbps);
        printf("  \"stream_latency_ns\": %.2f,\n", stream_latency_ns);
        printf("  \"zero_copy_mechanism\": \"Unified Memory / cudaHostAlloc Pinned / D3D12 SharedHandle\"\n");
        printf("}\n");
    } else {
        printf("====================================================================\n");
        printf("Tier 1 - CUDA 13.4 Native (sm_89) Benchmark Result\n");
        printf("Device: %s (%d SMs, Compute %d.%d)\n", prop.name, prop.multiProcessorCount, prop.major, prop.minor);
        printf("FP32 FMA Compute: %.2f GFLOPS (Latency: %.2f us)\n", compute_gflops, compute_latency_ns / 1000.0);
        printf("GDDR6X Triad Bandwidth: %.2f GB/s (Latency: %.2f us)\n", stream_bandwidth_gbps, stream_latency_ns / 1000.0);
        printf("====================================================================\n");
    }

    return 0;
}
