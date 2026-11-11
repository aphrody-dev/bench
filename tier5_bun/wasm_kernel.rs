#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_fma_compute(ptr: *mut f32, count: i32, iters: i32, alpha: f32, beta: f32) {
    unsafe {
        let slice = core::slice::from_raw_parts_mut(ptr, count as usize);
        for x in slice.iter_mut() {
            let mut v = *x;
            for _ in 0..iters {
                v = v * alpha + beta;
            }
            *x = v;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_triad(a: *const f32, b: *const f32, c: *mut f32, count: i32, alpha: f32) {
    unsafe {
        let sa = core::slice::from_raw_parts(a, count as usize);
        let sb = core::slice::from_raw_parts(b, count as usize);
        let sc = core::slice::from_raw_parts_mut(c, count as usize);
        for i in 0..count as usize {
            sc[i] = sa[i] + alpha * sb[i];
        }
    }
}
