use crate::object::{ObjectData, PyObject};
use crate::{GCResult, GarbageCollector};
use std::ffi::{c_int, c_void, c_char};

static mut GC: Option<GarbageCollector> = None;

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
            if gc.is_enabled() {
                1
            } else {
                0
            }
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
            std::ptr::copy_nonoverlapping(
                state_info.as_ptr(),
                buffer as *mut u8,
                bytes_to_copy,
            );
            *buffer.offset(bytes_to_copy as isize) = 0;

            GCReturnCode::Success
        } else {
            let error_msg = "GC not initialized";
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(
                error_msg.as_ptr(),
                buffer as *mut u8,
                bytes_to_copy,
            );
            *buffer.offset(bytes_to_copy as isize) = 0;

            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_track(obj_ptr: *mut c_void) -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
            if obj_ptr.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            let obj = PyObject::new("tracked_object".to_string(), ObjectData::None);
            let result = gc.track(obj);
            match result {
                Ok(_) => GCReturnCode::Success,
                Err(_) => GCReturnCode::ErrorAlreadyTracked,
            }
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_untrack(obj_ptr: *mut c_void) -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
            if obj_ptr.is_null() {
                return GCReturnCode::ErrorInternal;
            }

            let obj_id = crate::object::ObjectId::new();
            let result = gc.untrack(&obj_id);
            match result {
                Ok(_) => GCReturnCode::Success,
                Err(_) => GCReturnCode::ErrorNotTracked,
            }
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
            if gc.needs_collection() {
                1
            } else {
                0
            }
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

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_tracked(obj_ptr: *mut c_void) -> c_int {
    unsafe {
        if let Some(ref _gc) = GC {
            if obj_ptr.is_null() {
                return 0;
            }

            0
        } else {
            0
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