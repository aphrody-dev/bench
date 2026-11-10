use crate::hardware::QpcInstant;
use serde::{Deserialize, Serialize};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct TelemetryRecord {
    pub id: u64,
    pub timestamp_ns: u64,
    pub metric_value: f64,
    pub channel_id: u32,
    pub flags: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZeroCopyMetrics {
    pub test_name: String,
    pub record_count: usize,
    pub total_bytes: usize,
    pub win32_mmap_zerocopy_latency_ns: f64,
    pub win32_mmap_zerocopy_allocations: usize,
    pub win32_mmap_bytes_copied: usize,
    pub heap_deserialization_latency_ns: f64,
    pub heap_deserialization_allocations: usize,
    pub heap_bytes_copied: usize,
    pub speedup_factor: f64,
}

pub fn run_zerocopy_benchmark() -> ZeroCopyMetrics {
    const RECORD_COUNT: usize = 1_000_000;
    let total_bytes = RECORD_COUNT * std::mem::size_of::<TelemetryRecord>(); // 32 MB

    // 1. Create Win32 Paging-File backed Memory Mapping
    unsafe {
        let handle = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            null_mut(),
            PAGE_READWRITE,
            (total_bytes >> 32) as u32,
            (total_bytes & 0xFFFFFFFF) as u32,
            null_mut(),
        );
        if handle.is_null() {
            panic!("Win32 CreateFileMappingW failed");
        }

        let map_view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, total_bytes);
        if map_view.Value.is_null() {
            CloseHandle(handle);
            panic!("Win32 MapViewOfFile failed");
        }

        let mapped_slice = std::slice::from_raw_parts_mut(map_view.Value as *mut u8, total_bytes);

        // Pre-populate mapped buffer with structured records
        let sample = TelemetryRecord {
            id: 42,
            timestamp_ns: 123456789,
            metric_value: 3.1415926535,
            channel_id: 1,
            flags: 0x8000,
        };
        let sample_bytes = sample.as_bytes();
        for chunk in mapped_slice.chunks_exact_mut(std::mem::size_of::<TelemetryRecord>()) {
            chunk.copy_from_slice(sample_bytes);
        }

        // Benchmark Test A: Zero-Copy Cast via Google zerocopy (0 allocations, 0 copies)
        let mut min_zerocopy_ns = f64::MAX;
        for _ in 0..10 {
            let t0 = QpcInstant::now();
            let records = <[TelemetryRecord]>::ref_from_bytes(mapped_slice)
                .expect("zerocopy slice cast failed");
            let first_id = records[0].id;
            let last_id = records[RECORD_COUNT - 1].id;
            std::hint::black_box((first_id, last_id));
            let dt = t0.elapsed_nanos();
            if dt < min_zerocopy_ns {
                min_zerocopy_ns = dt;
            }
        }

        // Benchmark Test B: Standard Heap Deserialization (Heap allocation + full 32MB copy)
        let mut min_heap_ns = f64::MAX;
        for _ in 0..10 {
            let t0 = QpcInstant::now();
            // Allocate heap buffer and copy bytes
            let mut heap_bytes = vec![0u8; total_bytes];
            heap_bytes.copy_from_slice(mapped_slice);
            // Parse / convert into Vec<TelemetryRecord> on heap
            let mut heap_records = Vec::with_capacity(RECORD_COUNT);
            for chunk in heap_bytes.chunks_exact(std::mem::size_of::<TelemetryRecord>()) {
                let rec = TelemetryRecord::read_from_bytes(chunk).unwrap();
                heap_records.push(rec);
            }
            let first_id = heap_records[0].id;
            let last_id = heap_records[RECORD_COUNT - 1].id;
            std::hint::black_box((first_id, last_id));
            let dt = t0.elapsed_nanos();
            if dt < min_heap_ns {
                min_heap_ns = dt;
            }
        }

        UnmapViewOfFile(map_view);
        CloseHandle(handle);

        let speedup = min_heap_ns / min_zerocopy_ns.max(1.0);

        ZeroCopyMetrics {
            test_name: "Win32 MapViewOfFile + Google zerocopy vs Heap Deserialization".to_string(),
            record_count: RECORD_COUNT,
            total_bytes,
            win32_mmap_zerocopy_latency_ns: min_zerocopy_ns,
            win32_mmap_zerocopy_allocations: 0,
            win32_mmap_bytes_copied: 0,
            heap_deserialization_latency_ns: min_heap_ns,
            heap_deserialization_allocations: 2,
            heap_bytes_copied: total_bytes,
            speedup_factor: speedup,
        }
    }
}
