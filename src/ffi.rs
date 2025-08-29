use crate::object::{ObjectData, PyObject};
use crate::{GCResult, GarbageCollector};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" {
    fn PyList_New(size: isize) -> *mut c_void;
    fn PyList_SetItem(list: *mut c_void, index: isize, item: *mut c_void) -> c_int;
    fn PyList_GetItem(list: *mut c_void, index: isize) -> *mut c_void;
    fn PyList_Size(list: *mut c_void) -> isize;
    fn Py_IncRef(obj: *mut c_void);
    fn Py_DecRef(obj: *mut c_void);
}

static mut GC: Option<GarbageCollector> = None;
static AUTOMATIC_TRACKING: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OBJECT_REGISTRY: RefCell<HashMap<*mut c_void, PyObject>> = RefCell::new(HashMap::new());
    static REFCOUNT_CALLBACKS: RefCell<HashMap<*mut c_void, RefCountCallback>> = RefCell::new(HashMap::new());
    static REFERENCE_TRACKING: RefCell<HashMap<*mut c_void, HashSet<*mut c_void>>> = RefCell::new(HashMap::new());
    static UNCOLLECTABLE_OBJECTS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

type RefCountCallback = Box<dyn Fn(*mut c_void, i32) + Send + Sync>;

const PY_TPFLAGS_HAVE_GC: u64 = 0x00000020;

#[repr(C)]
struct PyObject_HEAD {
    ob_refcnt: usize,
    ob_type: *mut PyTypeObject,
}

#[repr(C)]
struct PyTypeObject {
    ob_refcnt: usize,
    ob_type: *mut PyTypeObject,
    ob_size: isize,
    tp_name: *const c_char,
    tp_basicsize: isize,
    tp_itemsize: isize,
    tp_dealloc: Option<unsafe extern "C" fn(*mut c_void)>,
    tp_print: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int>,
    tp_getattr: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    tp_setattr: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void) -> c_int>,
    tp_compare: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>,
    tp_repr: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    tp_as_number: *mut c_void,
    tp_as_sequence: *mut c_void,
    tp_as_mapping: *mut c_void,
    tp_hash: Option<unsafe extern "C" fn(*mut c_void) -> isize>,
    tp_call: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void>,
    tp_str: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    tp_getattro: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void>,
    tp_setattro: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> c_int>,
    tp_as_buffer: *mut c_void,
    tp_flags: u64,
    tp_doc: *const c_char,
    tp_traverse: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> c_int>,
    tp_clear: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    tp_richcompare: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> *mut c_void>,
    tp_weaklistoffset: isize,
    tp_iter: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    tp_iternext: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    tp_methods: *mut c_void,
    tp_members: *mut c_void,
    tp_getset: *mut c_void,
    tp_base: *mut PyTypeObject,
    tp_dict: *mut c_void,
    tp_descr_get:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void>,
    tp_descr_set: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> c_int>,
    tp_dictoffset: isize,
    tp_init: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> c_int>,
    tp_alloc: Option<unsafe extern "C" fn(*mut PyTypeObject, isize) -> *mut c_void>,
    tp_new:
        Option<unsafe extern "C" fn(*mut PyTypeObject, *mut c_void, *mut c_void) -> *mut c_void>,
    tp_free: Option<unsafe extern "C" fn(*mut c_void)>,
    tp_is_gc: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    tp_bases: *mut c_void,
    tp_mro: *mut c_void,
    tp_cache: *mut c_void,
    tp_subclasses: *mut c_void,
    tp_weaklist: *mut c_void,
    tp_del: Option<unsafe extern "C" fn(*mut c_void)>,
    tp_version_tag: c_uint,
    tp_finalize: Option<unsafe extern "C" fn(*mut c_void)>,
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
    OBJECT_REGISTRY.with(|registry| {
        let registry = registry.borrow();
        registry.contains_key(&obj_ptr)
    })
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

#[inline(always)]
fn register_refcount_callback(obj_ptr: *mut c_void, callback: RefCountCallback) {
    REFCOUNT_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().insert(obj_ptr, callback);
    });
}

#[inline(always)]
fn unregister_refcount_callback(obj_ptr: *mut c_void) {
    REFCOUNT_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().remove(&obj_ptr);
    });
}

#[inline(always)]
fn notify_refcount_change(obj_ptr: *mut c_void, delta: i32) {
    REFCOUNT_CALLBACKS.with(|callbacks| {
        if let Some(callback) = callbacks.borrow().get(&obj_ptr) {
            callback(obj_ptr, delta);
        }
    });
}

#[inline(always)]
fn add_reference(from_obj: *mut c_void, to_obj: *mut c_void) {
    REFERENCE_TRACKING.with(|refs| {
        let mut refs = refs.borrow_mut();
        refs.entry(from_obj).or_default().insert(to_obj);
    });
}

#[inline(always)]
fn remove_reference(from_obj: *mut c_void, to_obj: *mut c_void) {
    REFERENCE_TRACKING.with(|refs| {
        let mut refs = refs.borrow_mut();
        if let Some(references) = refs.get_mut(&from_obj) {
            references.remove(&to_obj);
            if references.is_empty() {
                refs.remove(&from_obj);
            }
        }
    });
}

#[inline(always)]
fn get_references(from_obj: *mut c_void) -> Vec<*mut c_void> {
    REFERENCE_TRACKING.with(|refs| {
        refs.borrow()
            .get(&from_obj)
            .map(|references| references.iter().copied().collect())
            .unwrap_or_default()
    })
}

#[inline(always)]
fn get_referrers(to_obj: *mut c_void) -> Vec<*mut c_void> {
    REFERENCE_TRACKING.with(|refs| {
        refs.borrow()
            .iter()
            .filter_map(|(from_obj, references)| references.contains(&to_obj).then_some(*from_obj))
            .collect()
    })
}

#[inline(always)]
unsafe fn create_python_list_from_objects(objects: Vec<*mut c_void>) -> *mut c_void {
    if objects.is_empty() {
        return std::ptr::null_mut();
    }

    let list_size = objects.len() as isize;
    let py_list = unsafe { PyList_New(list_size) };
    if py_list.is_null() {
        return std::ptr::null_mut();
    }

    for (index, obj_ptr) in objects.into_iter().enumerate() {
        if !obj_ptr.is_null() {
            unsafe {
                Py_IncRef(obj_ptr);
                if PyList_SetItem(py_list, index as isize, obj_ptr) != 0 {
                    Py_DecRef(obj_ptr);
                }
            }
        }
    }
    py_list
}

#[inline(always)]
fn add_uncollectable(obj_ptr: *mut c_void) {
    UNCOLLECTABLE_OBJECTS.with(|uncollectable| {
        if !uncollectable.borrow().contains(&obj_ptr) {
            uncollectable.borrow_mut().push(obj_ptr);
        }
    });
}

#[inline(always)]
fn remove_uncollectable(obj_ptr: *mut c_void) {
    UNCOLLECTABLE_OBJECTS.with(|uncollectable| {
        uncollectable.borrow_mut().retain(|&ptr| ptr != obj_ptr);
    });
}

#[inline(always)]
fn get_uncollectable_objects() -> Vec<*mut c_void> {
    UNCOLLECTABLE_OBJECTS.with(|uncollectable| uncollectable.borrow().clone())
}

#[inline(always)]
fn clear_uncollectable_objects() {
    UNCOLLECTABLE_OBJECTS.with(|uncollectable| uncollectable.borrow_mut().clear());
}

const COMMON_NAMES: [&str; 4] = ["tracked_ptr", "list", "dict", "tuple"];

#[inline(always)]
fn get_fast_object_name(ptr_addr: usize) -> &'static str {
    let index = ptr_addr & (COMMON_NAMES.len() - 1);
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
        AUTOMATIC_TRACKING.store(false, Ordering::Relaxed);
    }
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_cleanup() -> GCReturnCode {
    unsafe {
        with_object_registry(|reg| reg.clear());
        REFCOUNT_CALLBACKS.with(|callbacks| callbacks.borrow_mut().clear());
        REFERENCE_TRACKING.with(|refs| refs.borrow_mut().clear());
        clear_uncollectable_objects();

        GC = None;
        AUTOMATIC_TRACKING.store(false, Ordering::Relaxed);
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

/// Get GC state information as a string
///
/// # Safety
///
/// - `buffer` must be a valid pointer to a buffer of at least `buffer_size` bytes
/// - `buffer_size` must be greater than 0
/// - The buffer must be writable and not overlap with any other memory being accessed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_state_string(
    buffer: *mut c_char,
    buffer_size: usize,
) -> GCReturnCode {
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
            *buffer.add(bytes_to_copy) = 0;

            GCReturnCode::Success
        } else {
            let error_msg = "GC not initialized";
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.add(bytes_to_copy) = 0;

            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_track(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    if is_object_tracked(obj_ptr) {
        return GCReturnCode::ErrorAlreadyTracked;
    }

    let ptr_addr = obj_ptr as usize;
    let _obj_name = get_fast_object_name(ptr_addr);

    let obj = unsafe {
        let original_obj = &*(obj_ptr as *mut PyObject);
        original_obj.clone()
    };

    track_object_fast(obj_ptr, obj);
    GCReturnCode::Success
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
        if let Some(ref mut gc) = GC {
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
    if obj_ptr.is_null() {
        return 0;
    }

    is_object_tracked(obj_ptr) as c_int
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
pub extern "C" fn py_gc_add_reference(from_obj: *mut c_void, to_obj: *mut c_void) -> GCReturnCode {
    if from_obj.is_null() || to_obj.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    add_reference(from_obj, to_obj);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_remove_reference(
    from_obj: *mut c_void,
    to_obj: *mut c_void,
) -> GCReturnCode {
    if from_obj.is_null() || to_obj.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    remove_reference(from_obj, to_obj);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_mark_uncollectable(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    add_uncollectable(obj_ptr);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_unmark_uncollectable(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    remove_uncollectable(obj_ptr);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_uncollectable(obj_ptr: *mut c_void) -> c_int {
    if obj_ptr.is_null() {
        return 0;
    }

    UNCOLLECTABLE_OBJECTS.with(|uncollectable| {
        if uncollectable.borrow().contains(&obj_ptr) {
            1
        } else {
            0
        }
    })
}

/// Get information about a tracked object
///
/// # Safety
///
/// - `obj_ptr` must be a valid pointer to a tracked object or null
/// - `buffer` must be a valid pointer to a buffer of at least `buffer_size` bytes
/// - `buffer_size` must be greater than 0
/// - The buffer must be writable and not overlap with any other memory being accessed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_tracked_info(
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
                *buffer.add(bytes_to_copy) = 0;
                return GCReturnCode::ErrorInternal;
            }

            if !is_object_tracked(obj_ptr) {
                let error_msg = "Pointer not tracked";
                let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
                std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
                *buffer.add(bytes_to_copy) = 0;
                return GCReturnCode::ErrorNotTracked;
            }

            let obj_info = with_object_registry(|reg| {
                if let Some(obj) = reg.get(&obj_ptr) {
                    format!(
                        "Object: {} (ID: {}, Refs: {}, Ptr: {:p})",
                        obj.name,
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
            *buffer.add(bytes_to_copy) = 0;

            GCReturnCode::Success
        } else {
            let error_msg = "GC not initialized";
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.add(bytes_to_copy) = 0;
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
            println!("  Registry count: {registry_count}");

            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorInternal
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_enable_automatic_tracking() -> GCReturnCode {
    AUTOMATIC_TRACKING.store(true, Ordering::Relaxed);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_disable_automatic_tracking() -> GCReturnCode {
    AUTOMATIC_TRACKING.store(false, Ordering::Relaxed);
    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_automatic_tracking_enabled() -> c_int {
    if AUTOMATIC_TRACKING.load(Ordering::Relaxed) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_object_created(obj_ptr: *mut c_void) -> GCReturnCode {
    if !AUTOMATIC_TRACKING.load(Ordering::Relaxed) {
        return GCReturnCode::Success;
    }

    unsafe {
        if obj_ptr.is_null() {
            return GCReturnCode::ErrorInternal;
        }

        if is_object_tracked(obj_ptr) {
            return GCReturnCode::ErrorAlreadyTracked;
        }

        let py_obj = obj_ptr as *mut PyObject_HEAD;
        let py_type = (*py_obj).ob_type;
        let type_name = if !py_type.is_null() {
            let type_name_ptr = (*py_type).tp_name;
            if !type_name_ptr.is_null() {
                std::ffi::CStr::from_ptr(type_name_ptr)
                    .to_string_lossy()
                    .to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        let obj = PyObject::new_ffi(&type_name, ObjectData::None, obj_ptr);

        track_object_fast(obj_ptr, obj);

        register_refcount_callback(
            obj_ptr,
            Box::new(|obj_ptr, delta| {
                if delta < 0 && py_gc_get_refcount(obj_ptr) == 0 {
                    if let Some(ref gc) = GC {
                        gc.collect_if_needed().ok();
                    }
                }
            }),
        );

        GCReturnCode::Success
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_object_destroyed(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    unregister_refcount_callback(obj_ptr);

    if untrack_object_fast(obj_ptr) {
        GCReturnCode::Success
    } else {
        GCReturnCode::ErrorNotTracked
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_refcount_changed(
    obj_ptr: *mut c_void,
    old_count: c_int,
    new_count: c_int,
) -> GCReturnCode {
    if !AUTOMATIC_TRACKING.load(Ordering::Relaxed) {
        return GCReturnCode::Success;
    }

    unsafe {
        if obj_ptr.is_null() {
            return GCReturnCode::ErrorInternal;
        }

        let delta = new_count - old_count;
        notify_refcount_change(obj_ptr, delta);

        if new_count == 0 {
            if let Some(ref gc) = GC {
                gc.collect_if_needed().ok();
            }
        }

        GCReturnCode::Success
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_refcount(obj_ptr: *mut c_void) -> c_int {
    if obj_ptr.is_null() {
        return 0;
    }

    with_object_registry(|reg| {
        if let Some(obj) = reg.get(&obj_ptr) {
            obj.get_refcount() as c_int
        } else {
            unsafe {
                let py_obj = obj_ptr as *mut PyObject_HEAD;
                (*py_obj).ob_refcnt as c_int
            }
        }
    })
}

/// Set the reference count of an object
///
/// # Safety
///
/// - `obj_ptr` must be a valid pointer to a Python object or null
/// - The object must not be in an inconsistent state
/// - `refcount` must be non-negative
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_set_refcount(obj_ptr: *mut c_void, refcount: c_int) -> GCReturnCode {
    if obj_ptr.is_null() || refcount < 0 {
        return GCReturnCode::ErrorInternal;
    }

    let mut success = false;
    with_object_registry(|reg| {
        if let Some(obj) = reg.get_mut(&obj_ptr) {
            let current_refcount = obj.get_refcount();
            let target_refcount = refcount as usize;

            if target_refcount > current_refcount {
                for _ in 0..(target_refcount - current_refcount) {
                    obj.inc_ref();
                }
            } else if target_refcount < current_refcount {
                for _ in 0..(current_refcount - target_refcount) {
                    obj.dec_ref();
                }
            }

            success = true;
        } else {
            unsafe {
                let py_obj = obj_ptr as *mut PyObject_HEAD;
                let current_refcount = (*py_obj).ob_refcnt;
                let target_refcount = refcount as usize;

                if target_refcount > current_refcount {
                    for _ in 0..(target_refcount - current_refcount) {
                        Py_IncRef(obj_ptr);
                    }
                } else if target_refcount < current_refcount {
                    for _ in 0..(current_refcount - target_refcount) {
                        Py_DecRef(obj_ptr);
                    }
                }

                (*py_obj).ob_refcnt = target_refcount;
            }

            let ptr_addr = obj_ptr as usize;
            let type_name = get_fast_object_name(ptr_addr);
            let obj = PyObject::new_ffi(type_name, ObjectData::None, obj_ptr);
            reg.insert(obj_ptr, obj);
            success = true;
        }
    });

    if success {
        GCReturnCode::Success
    } else {
        GCReturnCode::ErrorInternal
    }
}

/// Get all tracked objects as a Python list
///
/// # Safety
///
/// - The returned pointer must be properly managed by the caller
/// - The caller is responsible for decrementing the reference count when done
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_objects() -> *mut c_void {
    with_object_registry(|reg| {
        let objects: Vec<*mut c_void> = reg.keys().copied().collect();
        unsafe { create_python_list_from_objects(objects) }
    })
}

/// Get objects that refer to the given object
///
/// # Safety
///
/// - `obj_ptr` must be a valid pointer to a tracked object or null
/// - The returned pointer must be properly managed by the caller
/// - The caller is responsible for decrementing the reference count when done
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_referrers(obj_ptr: *mut c_void) -> *mut c_void {
    if obj_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let referrers = get_referrers(obj_ptr);
    unsafe { create_python_list_from_objects(referrers) }
}

/// Get objects that the given object refers to
///
/// # Safety
///
/// - `obj_ptr` must be a valid pointer to a tracked object or null
/// - The returned pointer must be properly managed by the caller
/// - The caller is responsible for decrementing the reference count when done
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_referents(obj_ptr: *mut c_void) -> *mut c_void {
    if obj_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let references = get_references(obj_ptr);
    unsafe { create_python_list_from_objects(references) }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_is_tracked_python(obj_ptr: *mut c_void) -> c_int {
    if obj_ptr.is_null() {
        return 0;
    }

    unsafe {
        let py_obj = obj_ptr as *mut PyObject_HEAD;
        let py_type = (*py_obj).ob_type;
        if !py_type.is_null() {
            let flags = (*py_type).tp_flags;
            if (flags & PY_TPFLAGS_HAVE_GC) != 0 && is_object_tracked(obj_ptr) {
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
pub extern "C" fn py_gc_track_python(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    if is_object_tracked(obj_ptr) {
        return GCReturnCode::ErrorAlreadyTracked;
    }

    let type_name = unsafe {
        let py_obj = obj_ptr as *mut PyObject_HEAD;
        let py_type = (*py_obj).ob_type;
        if !py_type.is_null() {
            let type_name_ptr = (*py_type).tp_name;
            if !type_name_ptr.is_null() {
                std::ffi::CStr::from_ptr(type_name_ptr)
                    .to_string_lossy()
                    .to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        }
    };

    let obj = PyObject::new_ffi(&type_name, ObjectData::None, obj_ptr);

    track_object_fast(obj_ptr, obj);

    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_untrack_python(obj_ptr: *mut c_void) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    if untrack_object_fast(obj_ptr) {
        GCReturnCode::Success
    } else {
        GCReturnCode::ErrorNotTracked
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_collection_counts() -> *mut c_int {
    unsafe {
        if let Some(ref gc) = GC {
            let counts = Box::new([
                gc.get_generation_count(0).unwrap_or(0) as c_int,
                gc.get_generation_count(1).unwrap_or(0) as c_int,
                gc.get_generation_count(2).unwrap_or(0) as c_int,
            ]);
            Box::into_raw(counts) as *mut c_int
        } else {
            std::ptr::null_mut()
        }
    }
}

/// Free memory allocated for collection counts
///
/// # Safety
///
/// - `counts` must be a valid pointer previously returned by a GC function
/// - The pointer must not be used after this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_free_collection_counts(counts: *mut c_int) {
    if !counts.is_null() {
        unsafe {
            let _ = Box::from_raw(counts);
        }
    }
}

/// Get uncollectable objects as a Python list
///
/// # Safety
///
/// - The returned pointer must be properly managed by the caller
/// - The caller is responsible for decrementing the reference count when done
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_garbage() -> *mut c_void {
    let uncollectable = get_uncollectable_objects();
    unsafe { create_python_list_from_objects(uncollectable) }
}

/// Set the garbage list for uncollectable objects
///
/// # Safety
///
/// - `garbage_list` must be a valid pointer to a Python list or null
/// - The list must contain valid object pointers
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_set_garbage(garbage_list: *mut c_void) -> GCReturnCode {
    if garbage_list.is_null() {
        clear_uncollectable_objects();
        return GCReturnCode::Success;
    }

    clear_uncollectable_objects();

    unsafe {
        let list_size = PyList_Size(garbage_list);
        if list_size < 0 {
            return GCReturnCode::ErrorInternal;
        }

        for i in 0..list_size {
            let item = PyList_GetItem(garbage_list, i);
            if !item.is_null() {
                Py_IncRef(item);
                add_uncollectable(item);
            }
        }
    }

    GCReturnCode::Success
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_set_debug_flags(flags: c_int) -> GCReturnCode {
    unsafe {
        if let Some(ref mut gc) = GC {
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

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_debug_flags() -> c_int {
    unsafe {
        if let Some(ref gc) = GC {
            gc.get_debug() as c_int
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_has_finalizer(obj_ptr: *mut c_void) -> c_int {
    if obj_ptr.is_null() {
        return 0;
    }

    with_object_registry(|reg| {
        if let Some(obj) = reg.get(&obj_ptr) {
            if obj.has_finalizer { 1 } else { 0 }
        } else {
            0 // Object not tracked, so no finalizer
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_set_finalizer(obj_ptr: *mut c_void, has_finalizer: c_int) -> GCReturnCode {
    if obj_ptr.is_null() {
        return GCReturnCode::ErrorInternal;
    }

    with_object_registry(|reg| {
        if let Some(obj) = reg.get_mut(&obj_ptr) {
            obj.set_finalizer(has_finalizer != 0);
            GCReturnCode::Success
        } else {
            GCReturnCode::ErrorNotTracked
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn py_gc_get_object_size(obj_ptr: *mut c_void) -> c_int {
    if obj_ptr.is_null() {
        return 0;
    }

    with_object_registry(|reg| {
        if let Some(obj) = reg.get(&obj_ptr) {
            match &obj.data {
                ObjectData::Integer(_) => 8,
                ObjectData::Float(_) => 8,
                ObjectData::String(s) => s.len() as c_int,
                ObjectData::List(l) => (l.len() * std::mem::size_of::<PyObject>()) as c_int,
                ObjectData::Dict(d) => {
                    (d.len() * std::mem::size_of::<(PyObject, PyObject)>()) as c_int
                }
                ObjectData::Custom(_) => std::mem::size_of::<*mut c_void>() as c_int,
                ObjectData::None => 0,
            }
        } else {
            0
        }
    })
}

/// Get the type name of an object
///
/// # Safety
///
/// - `obj_ptr` must be a valid pointer to a tracked object or null
/// - `buffer` must be a valid pointer to a buffer of at least `buffer_size` bytes
/// - `buffer_size` must be greater than 0
/// - The buffer must be writable and not overlap with any other memory being accessed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn py_gc_get_object_type_name(
    obj_ptr: *mut c_void,
    buffer: *mut c_char,
    buffer_size: usize,
) -> GCReturnCode {
    if buffer.is_null() || buffer_size == 0 {
        return GCReturnCode::ErrorInternal;
    }

    if obj_ptr.is_null() {
        let error_msg = "NULL pointer";
        unsafe {
            let bytes_to_copy = std::cmp::min(error_msg.len(), buffer_size - 1);
            std::ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, bytes_to_copy);
            *buffer.add(bytes_to_copy) = 0;
        }
        return GCReturnCode::ErrorInternal;
    }

    let type_name = with_object_registry(|reg| {
        if let Some(obj) = reg.get(&obj_ptr) {
            obj.name.clone()
        } else {
            "unknown".to_string()
        }
    });

    unsafe {
        let bytes_to_copy = std::cmp::min(type_name.len(), buffer_size - 1);
        std::ptr::copy_nonoverlapping(type_name.as_ptr(), buffer as *mut u8, bytes_to_copy);
        *buffer.add(bytes_to_copy) = 0;
    }

    GCReturnCode::Success
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

        assert_eq!(py_gc_cleanup() as i32, GCReturnCode::Success as i32);
    }

    #[test]
    fn test_gc_collection() {
        assert_eq!(py_gc_init() as i32, GCReturnCode::Success as i32);

        let result = py_gc_collect();
        assert_eq!(result as i32, GCReturnCode::Success as i32);

        assert_eq!(py_gc_cleanup() as i32, GCReturnCode::Success as i32);
    }

    #[test]
    fn test_finalizer_behavior() {
        assert_eq!(py_gc_init() as i32, GCReturnCode::Success as i32);

        let obj1 = PyObject::new("regular_obj".to_string(), ObjectData::Integer(42));
        let obj1_ptr = Box::into_raw(Box::new(obj1)) as *mut c_void;

        assert_eq!(py_gc_track(obj1_ptr) as i32, GCReturnCode::Success as i32);

        assert_eq!(py_gc_has_finalizer(obj1_ptr), 0);

        assert_eq!(
            py_gc_set_finalizer(obj1_ptr, 1) as i32,
            GCReturnCode::Success as i32
        );

        assert_eq!(py_gc_has_finalizer(obj1_ptr), 1);

        let obj2 = PyObject::new_with_finalizer(
            "finalizer_obj".to_string(),
            ObjectData::String("test".to_string()),
        );
        let obj2_ptr = Box::into_raw(Box::new(obj2)) as *mut c_void;

        assert_eq!(py_gc_track(obj2_ptr) as i32, GCReturnCode::Success as i32);

        assert_eq!(py_gc_has_finalizer(obj2_ptr), 1);

        unsafe {
            let _ = Box::from_raw(obj1_ptr as *mut PyObject);
            let _ = Box::from_raw(obj2_ptr as *mut PyObject);
        }

        assert_eq!(py_gc_cleanup() as i32, GCReturnCode::Success as i32);
    }
}
