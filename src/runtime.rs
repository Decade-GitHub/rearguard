use core::ffi::c_void;

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn memcmp(left: *const c_void, right: *const c_void, length: usize) -> i32 {
    let left = left.cast::<u8>();
    let right = right.cast::<u8>();
    for index in 0..length {
        let a = unsafe { core::ptr::read_volatile(left.add(index)) };
        let b = unsafe { core::ptr::read_volatile(right.add(index)) };
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn memcpy(
    destination: *mut c_void,
    source: *const c_void,
    length: usize,
) -> *mut c_void {
    let destination = destination.cast::<u8>();
    let source = source.cast::<u8>();
    for index in 0..length {
        let byte = unsafe { core::ptr::read_volatile(source.add(index)) };
        unsafe { core::ptr::write_volatile(destination.add(index), byte) };
    }
    destination.cast()
}

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn memset(destination: *mut c_void, value: i32, length: usize) -> *mut c_void {
    let destination = destination.cast::<u8>();
    for index in 0..length {
        unsafe { core::ptr::write_volatile(destination.add(index), value as u8) };
    }
    destination.cast()
}
