use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(usize);

impl ObjectId {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        ObjectId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PyGCHead {
    pub _gc_next: usize,

    pub _gc_prev: usize,

    gc_refs: isize,

    collecting: bool,

    finalized: bool,
}

impl PyGCHead {
    pub fn new() -> Self {
        Self {
            _gc_next: 0,
            _gc_prev: 0,
            gc_refs: 0,
            collecting: false,
            finalized: false,
        }
    }

    pub fn get_refs(&self) -> isize {
        self.gc_refs
    }

    pub fn set_refs(&mut self, refs: isize) {
        self.gc_refs = refs;
    }

    pub fn set_collecting(&mut self) {
        self.collecting = true;
        self._gc_prev |= 2;
    }

    pub fn clear_collecting(&mut self) {
        self.collecting = false;
        self._gc_prev &= !2;
    }

    pub fn is_collecting(&self) -> bool {
        self.collecting
    }

    pub fn set_finalized(&mut self) {
        self.finalized = true;
        self._gc_prev |= 1;
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    pub fn is_tracked(&self) -> bool {
        self._gc_next != 0
    }
}

impl Default for PyGCHead {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ObjectData {
    Integer(i64),

    String(String),

    List(Vec<PyObject>),

    Dict(Vec<(PyObject, PyObject)>),

    Tuple(Vec<PyObject>),

    Set(Vec<PyObject>),

    Custom(Arc<dyn Any + Send + Sync>),

    None,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct PyObject {
    pub id: ObjectId,
    pub gc_tracked: bool,
    pub has_finalizer: bool,
    pub refcount: Arc<AtomicUsize>,

    pub gc_head: Option<PyGCHead>,

    pub type_name: String,
    pub data: Arc<RwLock<ObjectData>>,
    pub original_ptr: Option<*mut std::ffi::c_void>,
}

impl PyObject {
    pub fn new(type_name: String, data: ObjectData) -> Self {
        Self {
            id: ObjectId::new(),
            type_name,
            data: Arc::new(RwLock::new(data)),
            refcount: Arc::new(AtomicUsize::new(1)),
            gc_tracked: false,
            gc_head: None,
            has_finalizer: false,
            original_ptr: None,
        }
    }

    pub fn new_ffi(type_name: &str, data: ObjectData, ptr: *mut std::ffi::c_void) -> Self {
        Self {
            id: ObjectId::new(),
            type_name: type_name.to_string(),
            data: Arc::new(RwLock::new(data)),
            refcount: Arc::new(AtomicUsize::new(1)),
            gc_tracked: false,
            gc_head: None,
            has_finalizer: false,
            original_ptr: Some(ptr),
        }
    }

    pub fn new_with_finalizer(type_name: String, data: ObjectData) -> Self {
        Self {
            id: ObjectId::new(),
            type_name,
            data: Arc::new(RwLock::new(data)),
            refcount: Arc::new(AtomicUsize::new(1)),
            gc_tracked: false,
            gc_head: None,
            has_finalizer: true,
            original_ptr: None,
        }
    }

    pub fn new_with_ptr(type_name: String, data: ObjectData, ptr: *mut std::ffi::c_void) -> Self {
        Self {
            id: ObjectId::new(),
            type_name,
            data: Arc::new(RwLock::new(data)),
            refcount: Arc::new(AtomicUsize::new(1)),
            gc_tracked: false,
            gc_head: None,
            has_finalizer: false,
            original_ptr: Some(ptr),
        }
    }

    pub fn get_refcount(&self) -> usize {
        self.refcount.load(Ordering::Relaxed)
    }

    pub fn incref(&self) -> usize {
        self.refcount.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn decref(&self) -> usize {
        let current = self.refcount.fetch_sub(1, Ordering::Relaxed);
        if current == 0 {
            panic!("Reference count went below 0");
        }
        current - 1
    }

    pub fn should_track(&self) -> bool {
        matches!(
            &*self.data.try_read().unwrap(),
            ObjectData::List(_)
                | ObjectData::Dict(_)
                | ObjectData::Tuple(_)
                | ObjectData::Set(_)
                | ObjectData::Custom(_)
        )
    }

    pub fn matches_ptr(&self, ptr: *mut std::ffi::c_void) -> bool {
        self.original_ptr
            .map(|orig_ptr| orig_ptr == ptr)
            .unwrap_or(false)
    }

    pub fn get_size(&self) -> usize {
        match &*self.data.try_read().unwrap() {
            ObjectData::Integer(_) => 8,
            ObjectData::String(s) => s.len(),
            ObjectData::List(l) => l.len() * std::mem::size_of::<PyObject>(),
            ObjectData::Dict(d) => d.len() * std::mem::size_of::<(PyObject, PyObject)>(),
            ObjectData::Tuple(t) => t.len() * std::mem::size_of::<PyObject>(),
            ObjectData::Set(s) => s.len() * std::mem::size_of::<PyObject>(),
            ObjectData::Custom(_) => std::mem::size_of::<Arc<dyn Any + Send + Sync>>(),
            ObjectData::None => 0,
        }
    }
}

impl PartialEq for PyObject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PyObject {}

impl std::hash::Hash for PyObject {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Default for PyObject {
    fn default() -> Self {
        Self::new("object".to_string(), ObjectData::None)
    }
}

pub struct PyObjectPtr {
    ptr: *mut PyObject,
}

impl PyObjectPtr {
    /// Creates a new `PyObjectPtr` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid pointer to a `PyObject` or `null`.
    /// If `ptr` is not null, it must point to a valid, initialized `PyObject` instance.
    pub unsafe fn new(ptr: *mut PyObject) -> Self {
        Self { ptr }
    }

    pub fn as_ptr(&self) -> *mut PyObject {
        self.ptr
    }

    /// Returns a reference to the underlying `PyObject` if the pointer is not null.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `self.ptr` points to a valid, initialized `PyObject` instance.
    /// This function assumes the pointer is valid when not null.
    pub unsafe fn as_ref(&self) -> Option<&PyObject> {
        if self.ptr.is_null() {
            None
        } else {
            unsafe { Some(&*self.ptr) }
        }
    }

    /// Returns a mutable reference to the underlying `PyObject` if the pointer is not null.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `self.ptr` points to a valid, initialized `PyObject` instance.
    /// This function assumes the pointer is valid when not null.
    pub unsafe fn as_mut(&mut self) -> Option<&mut PyObject> {
        if self.ptr.is_null() {
            None
        } else {
            unsafe { Some(&mut *self.ptr) }
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl Clone for PyObjectPtr {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for PyObjectPtr {}

impl Default for PyObjectPtr {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
    }
}
