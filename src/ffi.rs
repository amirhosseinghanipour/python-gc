use crate::object::{ObjectData, PyObject};
use crate::{GCResult, GarbageCollector};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void};

static mut GC: Option<GarbageCollector> = None;


thread_local! {
    static OBJECT_REGISTRY: RefCell<HashMap<*mut c_void, PyObject>> = RefCell::new(HashMap::new());
}


#[inline(always)]
fn with_object_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<*mut c_void, PyObject>) -> R,
{
    OBJECT_REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        f(&mut registry)
    })
}


#[inline(always)]
fn is_object_tracked(obj_ptr: *mut c_void) -> bool {
    OBJECT_REGISTRY.with(|registry| registry.borrow().contains_key(&obj_ptr))
}

#[inline(always)]
fn track_object_fast(obj_ptr: *mut c_void, obj: PyObject) {
    OBJECT_REGISTRY.with(|registry| {
        registry.borrow_mut().insert(obj_ptr, obj);
    });
}

#[inline(always)]
fn untrack_object_fast(obj_ptr: *mut c_void) -> bool {
    OBJECT_REGISTRY.with(|registry| registry.borrow_mut().remove(&obj_ptr).is_some())
}


const COMMON_NAMES: [&str; 4] = ["tracked_ptr", "list", "dict", "tuple"];


#[inline(always)]
fn get_fast_object_name(ptr_addr: usize) -> &'static str {
    
    let index = ptr_addr % COMMON_NAMES.len();
    COMMON_NAMES[index]
}

#[repr(C)]
pub enum GCReturnCode {
    Success = 0,
    ErrorAlreadyTracked = -1,
    ErrorNotTracked = -2,
    ErrorCollectionInProgress = -3,
    ErrorInvalidGeneration = -4,
    ErrorInternal = -5,
}

impl From<GCResult<()>> for GCReturnCode {
    fn from(result: GCResult<()>) -> Self {
        match result {
            Ok(_) => GCReturnCode::Success,
            Err(e) => match e {
                crate::error::GCError::AlreadyTracked => GCReturnCode::ErrorAlreadyTracked,
                crate::error::GCError::NotTracked => GCReturnCode::ErrorNotTracked,
                crate::error::GCError::CollectionInProgress => {
                    GCReturnCode::ErrorCollectionInProgress
                }
                crate::error::GCError::InvalidGeneration(_) => GCReturnCode::ErrorInvalidGeneration,
                _ => GCReturnCode::ErrorInternal,
            },
        }
    }
}

impl From<GCResult<usize>> for GCReturnCode {
    fn from(result: GCResult<usize>) -> Self {
        match result {
            Ok(_) => GCReturnCode::Success,
            Err(e) => match e {
                crate::error::GCError::AlreadyTracked => GCReturnCode::ErrorAlreadyTracked,
                crate::error::GCError::NotTracked => GCReturnCode::ErrorNotTracked,
                crate::error::GCError::CollectionInProgress => {
                    GCReturnCode::ErrorCollectionInProgress
                }
                crate::error::GCError::InvalidGeneration(_) => GCReturnCode::ErrorInvalidGeneration,
                _ => GCReturnCode::ErrorInternal,
            },
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_init() -> GCReturnCode {
    unsafe {
        GC = Some(GarbageCollector::new());
    }
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_cleanup() -> GCReturnCode {
    unsafe {
        GC = None;
    }
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_enable() -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
            gc.enable();
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_disable() -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
            gc.disable();
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_enabled() -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            if gc.is_enabled() { 1 } else { 0 }
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_initialized() -> c_int {
    unsafe {
        match GC {
            Some(_) => 1,
            None => 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_state_string(buffer: *mut c_char, buffer_size: usize) -> GCReturnCode {
    if buffer.is_null() || buffer_size == 0 {
        return GCReturnCode::ErrorInternal;
    }

    unsafe {
        if let Some(ref gc) = GC {
            let state_info = format!(
                "GC State: enabled={}, tracked={}, gen0={}, gen1={}, gen2={}, uncollectable={}",
                gc.is_enabled(),
                gc.get_count(),
                gc.get_generation_count(0).unwrap_or(0),
                gc.get_generation_count(1).unwrap_or(0),
                gc.get_generation_count(2).unwrap_or(0),
                gc.get_uncollectable().len()
            );

            let bytes_to_copy = std::cmp::min(state_info.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(state_info.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.offset(bytes_to_copy as isize) = 0;

            GCReturnCode::Success
        } else {
            let error_msg = "GC not initialized";
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.offset(bytes_to_copy as isize) = 0;

            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_track(obj_ptr: *mut c_void) -> GCReturnCode {
    unsafe {
        if let Some(ref mut _gc) = GC {
            if obj_ptr.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            
            if is_object_tracked(obj_ptr) {
                return GCReturnCode::ErrorAlreadyTracked;
            }

            
            let ptr_addr = obj_ptr as usize;
            let obj_name = get_fast_object_name(ptr_addr);

            
            let obj = PyObject::new_ffi(obj_name, ObjectData::None, obj_ptr);

            // storing in registry without GC tracking to avoid sync issues
            track_object_fast(obj_ptr, obj);
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_untrack(obj_ptr: *mut c_void) -> GCReturnCode {
    unsafe {
        if let Some(ref mut _gc) = GC {
            if obj_ptr.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            
            if !untrack_object_fast(obj_ptr) {
                return GCReturnCode::ErrorNotTracked;
            }

            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_collect_generation(generation: c_int) -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            if !(0..=2).contains(&generation) {
                return GCReturnCode::ErrorInvalidGeneration;
            }

            gc.collect_generation(generation as usize).into()
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_collect() -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            gc.collect().into()
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_needs_collection() -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            if gc.needs_collection() { 1 } else { 0 }
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_collect_if_needed() -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            gc.collect_if_needed().into()
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_count() -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            gc.get_count() as c_int
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_generation_count(generation: c_int) -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            if !(0..=2).contains(&generation) {
                return -1;
            }

            gc.get_generation_count(generation as usize).unwrap_or(0) as c_int
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_set_threshold(generation: c_int, threshold: c_int) -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
            if !(0..=2).contains(&generation) || threshold < 0 {
                return GCReturnCode::ErrorInvalidGeneration;
            }

            gc.set_threshold(generation as usize, threshold as usize)
                .into()
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_threshold(generation: c_int) -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            if !(0..=2).contains(&generation) {
                return -1;
            }

            gc.get_threshold(generation as usize).unwrap_or(0) as c_int
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_set_debug(flags: c_int) -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            if flags < 0 {
                return GCReturnCode::ErrorInternal;
            }

            gc.set_debug(flags as u32);
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[repr(C)]
pub struct GCStats {
    pub total_tracked: c_int,
    pub generation_counts: [c_int; 3],
    pub uncollectable: c_int,
}

/// Retrieves garbage collection statistics.
///
/// # Safety
///
/// The caller must ensure that `stats` is a valid pointer to a `GCStats` struct.
/// The function will write to the memory pointed to by `stats`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_stats(stats: *mut GCStats) -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            if stats.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            let rust_stats = gc.get_stats();
            *stats = GCStats {
                total_tracked: rust_stats.total_tracked as c_int,
                generation_counts: [
                    rust_stats.generation_counts[0] as c_int,
                    rust_stats.generation_counts[1] as c_int,
                    rust_stats.generation_counts[2] as c_int,
                ],
                uncollectable: rust_stats.uncollectable as c_int,
            };

            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_tracked(obj_ptr: *mut c_void) -> c_int {
    unsafe {
        if let Some(ref _gc) = GC {
            if obj_ptr.is_null() {
                return 0;
            }

            
            if is_object_tracked(obj_ptr) {
                return 1;
            }

            return 0;
        } else {
            return 0;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_uncollectable_count() -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            gc.get_uncollectable().len() as c_int
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_registry_count() -> c_int {
    with_object_registry(|reg| reg.len() as c_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_clear_uncollectable() -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            gc.clear_uncollectable();
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_clear_registry() -> GCReturnCode {
    with_object_registry(|reg| {
        reg.clear();
        GCReturnCode::Success
    });
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_tracked_info(
    obj_ptr: *mut c_void,
    buffer: *mut c_char,
    buffer_size: usize,
) -> GCReturnCode {
    if buffer.is_null() || buffer_size == 0 {
        return GCReturnCode::ErrorInternal;
    }

    unsafe {
        if let Some(ref _gc) = GC {
            if obj_ptr.is_null() {
                let error_msg = "NULL pointer";
                let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
                std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
                *buffer.offset(bytes_to_copy as isize) = 0;
                return GCReturnCode::ErrorInternal;
            }

            
            if !is_object_tracked(obj_ptr) {
                let error_msg = "Pointer not tracked";
                let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
                std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
                *buffer.offset(bytes_to_copy as isize) = 0;
                return GCReturnCode::ErrorNotTracked;
            }

            
            let obj_info = with_object_registry(|reg| {
                if let Some(obj) = reg.get(&obj_ptr) {
                    format!(
                        "Object: {} (ID: {}, Refs: {}, Ptr: {:p})",
                        obj.type_name,
                        obj.id.as_usize(),
                        obj.get_refcount(),
                        obj_ptr
                    )
                } else {
                    "Object not found".to_string()
                }
            });

            let bytes_to_copy = std::cmp::min(obj_info.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(obj_info.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.offset(bytes_to_copy as isize) = 0;

            GCReturnCode::Success
        } else {
            let error_msg = "GC not initialized";
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.offset(bytes_to_copy as isize) = 0;
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_debug_untrack(obj_ptr: *mut c_void) -> GCReturnCode {
    unsafe {
        if let Some(ref mut _gc) = GC {
            if obj_ptr.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            
            if !untrack_object_fast(obj_ptr) {
                return GCReturnCode::ErrorNotTracked;
            }

            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_debug_state() -> GCReturnCode {
    unsafe {
        if let Some(ref gc) = GC {
            
            let stats = gc.get_stats();
            println!("GC Debug State:");
            println!("  Total tracked: {}", stats.total_tracked);
            println!("  Generation 0: {}", stats.generation_counts[0]);
            println!("  Generation 1: {}", stats.generation_counts[1]);
            println!("  Generation 2: {}", stats.generation_counts[2]);
            println!("  Uncollectable: {}", stats.uncollectable);

            
            let registry_count = with_object_registry(|reg| reg.len());
            println!("  Registry count: {}", registry_count);

            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_init_cleanup() {
        assert_eq!(py_gc_init() as i32, GCReturnCode::Success as i32);
        assert_eq!(py_gc_cleanup() as i32, GCReturnCode::Success as i32);
    }

    #[test]
    fn test_gc_enable_disable() {
        assert_eq!(py_gc_init() as i32, GCReturnCode::Success as i32);

        assert_eq!(py_gc_disable() as i32, GCReturnCode::Success as i32);
        assert_eq!(py_gc_is_enabled(), 0);

        assert_eq!(py_gc_enable() as i32, GCReturnCode::Success as i32);
        assert_eq!(py_gc_is_enabled(), 1);

        py_gc_cleanup();
    }

    #[test]
    fn test_gc_collection() {
        assert_eq!(py_gc_init() as i32, GCReturnCode::Success as i32);

        assert_eq!(py_gc_collect() as i32, GCReturnCode::Success as i32);
        assert_eq!(
            py_gc_collect_generation(0) as i32,
            GCReturnCode::Success as i32
        );

        py_gc_cleanup();
    }
}
