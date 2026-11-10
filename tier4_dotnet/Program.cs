using System;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.X86;
using System.Threading.Tasks;

class Program
{
    static void Main(string[] args)
    {
        bool jsonOutput = false;
        foreach (var arg in args)
        {
            if (arg == "--json") jsonOutput = true;
        }

        int threadCount = Environment.ProcessorCount;

        // Compute workload: 4M floats
        const int computeN = 4194304;
        const int fmaIters = 64;
        const double totalComputeFlops = (double)computeN * 4.0 * fmaIters * 2.0;
        float[] computeBuf = new float[computeN];
        Array.Fill(computeBuf, 1.0f);

        // Streaming workload: 33.5M floats per array = 134 MB each
        const int streamN = 33554432;
        const long streamBytes = (long)streamN * sizeof(float);
        const double totalStreamBytes = (double)streamBytes * 3.0;
        float[] a = new float[streamN];
        float[] b = new float[streamN];
        float[] c = new float[streamN];
        Array.Fill(a, 1.0f);
        Array.Fill(b, 2.0f);

        // Warmup JIT / AOT
        RunCompute(computeBuf, computeN, 8, threadCount);
        RunStream(a, b, c, 1024, threadCount);

        // Compute Benchmark
        double minComputeSec = 1e9;
        for (int run = 0; run < 5; run++)
        {
            long t0 = Stopwatch.GetTimestamp();
            RunCompute(computeBuf, computeN, fmaIters, threadCount);
            long t1 = Stopwatch.GetTimestamp();
            double dt = (double)(t1 - t0) / Stopwatch.Frequency;
            if (dt < minComputeSec) minComputeSec = dt;
        }

        double computeGflops = (totalComputeFlops / minComputeSec) / 1e9;
        double computeLatencyNs = minComputeSec * 1e9;

        // Streaming Benchmark
        double minStreamSec = 1e9;
        for (int run = 0; run < 5; run++)
        {
            long t0 = Stopwatch.GetTimestamp();
            RunStream(a, b, c, streamN, threadCount);
            long t1 = Stopwatch.GetTimestamp();
            double dt = (double)(t1 - t0) / Stopwatch.Frequency;
            if (dt < minStreamSec) minStreamSec = dt;
        }

        double streamBandwidthGbps = (totalStreamBytes / minStreamSec) / 1e9;
        double streamLatencyNs = minStreamSec * 1e9;

        // Zero-Copy Span<T> Benchmark
        byte[] rawBytes = new byte[32 * 1024 * 1024]; // 32 MB
        long zt0 = Stopwatch.GetTimestamp();
        Span<float> zeroCopyFloats = MemoryMarshal.Cast<byte, float>(rawBytes.AsSpan());
        float dummyZero = zeroCopyFloats[0];
        long zt1 = Stopwatch.GetTimestamp();
        double zeroCopyNs = (double)(zt1 - zt0) * 1e9 / Stopwatch.Frequency;

        if (jsonOutput)
        {
            var inv = System.Globalization.CultureInfo.InvariantCulture;
            Console.WriteLine("{");
            Console.WriteLine($"  \"tier\": \"Tier 4 - .NET 11 SIMD Vector256 / Span<T>\",");
            Console.WriteLine($"  \"runtime_version\": \"{Environment.Version}\",");
            Console.WriteLine($"  \"threads\": {threadCount},");
            Console.WriteLine($"  \"fma_gflops\": {computeGflops.ToString("F2", inv)},");
            Console.WriteLine($"  \"fma_latency_ns\": {computeLatencyNs.ToString("F2", inv)},");
            Console.WriteLine($"  \"stream_bandwidth_gbps\": {streamBandwidthGbps.ToString("F2", inv)},");
            Console.WriteLine($"  \"stream_latency_ns\": {streamLatencyNs.ToString("F2", inv)},");
            Console.WriteLine($"  \"zero_copy_latency_ns\": {zeroCopyNs.ToString("F2", inv)},");
            Console.WriteLine($"  \"zero_copy_mechanism\": \"Span<T> / MemoryMarshal.Cast (0 allocations, 0 copies)\"");
            Console.WriteLine("}");
        }
        else
        {
            Console.WriteLine("====================================================================");
            Console.WriteLine($"Tier 4 - .NET 11 SIMD Vector256 / Span<T> Benchmark Result");
            Console.WriteLine($"Threads: {threadCount} (Intel i7-13700F)");
            Console.WriteLine($"FP32 FMA Compute: {computeGflops:F2} GFLOPS (Latency: {computeLatencyNs / 1000.0:F2} us)");
            Console.WriteLine($"DDR5-5800 Triad Bandwidth: {streamBandwidthGbps:F2} GB/s (Latency: {streamLatencyNs / 1000.0:F2} us)");
            Console.WriteLine($"Span Zero-Copy Slicing Latency: {zeroCopyNs:F2} ns (0 allocations)");
            Console.WriteLine("====================================================================");
        }
    }

    [MethodImpl(MethodImplOptions.AggressiveOptimization)]
    static void RunCompute(float[] data, int n, int iters, int threads)
    {
        int vecCount = n / 8;
        int chunk = vecCount / threads;

        Parallel.For(0, threads, t =>
        {
            int start = t * chunk;
            int end = (t == threads - 1) ? vecCount : start + chunk;

            unsafe
            {
                fixed (float* pData = &data[0])
                {
                    Vector256<float> a = Vector256.Create(1.0001f);
                    Vector256<float> b = Vector256.Create(0.0001f);
                    Vector256<float> c1 = Vector256.Create(0.1f);
                    Vector256<float> c2 = Vector256.Create(0.2f);
                    Vector256<float> c3 = Vector256.Create(0.3f);

                    for (int i = start; i < end; i++)
                    {
                        float* ptr = pData + i * 8;
                        Vector256<float> v0 = Vector256.Load(ptr);
                        Vector256<float> v1 = v0 + c1;
                        Vector256<float> v2 = v0 + c2;
                        Vector256<float> v3 = v0 + c3;

                        for (int it = 0; it < iters; it++)
                        {
                            if (Fma.IsSupported)
                            {
                                v0 = Fma.MultiplyAdd(v0, a, b);
                                v1 = Fma.MultiplyAdd(v1, a, b);
                                v2 = Fma.MultiplyAdd(v2, a, b);
                                v3 = Fma.MultiplyAdd(v3, a, b);
                            }
                            else
                            {
                                v0 = (v0 * a) + b;
                                v1 = (v1 * a) + b;
                                v2 = (v2 * a) + b;
                                v3 = (v3 * a) + b;
                            }
                        }

                        Vector256<float> sum = (v0 + v1) + (v2 + v3);
                        sum.Store(ptr);
                    }
                }
            }
        });
    }

    [MethodImpl(MethodImplOptions.AggressiveOptimization)]
    static void RunStream(float[] a, float[] b, float[] c, int n, int threads)
    {
        int vecCount = n / 8;
        int chunk = vecCount / threads;

        Parallel.For(0, threads, t =>
        {
            int start = t * chunk;
            int end = (t == threads - 1) ? vecCount : start + chunk;

            unsafe
            {
                fixed (float* pA = &a[0], pB = &b[0], pC = &c[0])
                {
                    Vector256<float> alpha = Vector256.Create(3.0f);

                    for (int i = start; i < end; i++)
                    {
                        Vector256<float> va = Vector256.Load(pA + i * 8);
                        Vector256<float> vb = Vector256.Load(pB + i * 8);
                        Vector256<float> vc = Fma.IsSupported ? Fma.MultiplyAdd(vb, alpha, va) : (vb * alpha) + va;
                        vc.Store(pC + i * 8);
                    }
                }
            }
        });
    }
}
