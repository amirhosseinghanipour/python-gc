use std::ffi::c_void;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId {
    pub id: usize,
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectId {
    pub fn new() -> Self {
        static mut COUNTER: usize = 0;
        unsafe {
            COUNTER += 1;
            Self { id: COUNTER }
        }
    }

    pub fn as_usize(&self) -> usize {
        self.id
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PyGCHead {
    pub _gc_next: usize,
    pub _gc_prev: usize,
}

impl Default for PyGCHead {
    fn default() -> Self {
        Self::new()
    }
}

impl PyGCHead {
    pub fn new() -> Self {
        Self {
            _gc_next: 0,
            _gc_prev: 0,
        }
    }

    pub fn set_next(&mut self, next: *mut PyGCHead) {
        self._gc_next = next as usize;
    }

    pub fn get_next(&self) -> *mut PyGCHead {
        self._gc_next as *mut PyGCHead
    }

    pub fn set_prev(&mut self, prev: *mut PyGCHead) {
        self._gc_prev = (self._gc_prev & 0x3) | (prev as usize);
    }

    pub fn get_prev(&self) -> *mut PyGCHead {
        (self._gc_prev & !0x3) as *mut PyGCHead
    }

    pub fn set_refs(&mut self, refs: isize) {
        self._gc_prev = (self._gc_prev & 0x3) | ((refs as usize) << 2);
    }

    pub fn get_refs(&self) -> isize {
        ((self._gc_prev >> 2) & 0x3FFFFFFFFFFFFFFF) as isize
    }

    pub fn set_collecting(&mut self) {
        self._gc_prev |= 0x2;
    }

    pub fn clear_collecting(&mut self) {
        self._gc_prev &= !0x2;
    }

    pub fn is_collecting(&self) -> bool {
        (self._gc_prev & 0x2) != 0
    }

    pub fn set_finalized(&mut self) {
        self._gc_prev |= 0x1;
    }

    pub fn is_finalized(&self) -> bool {
        (self._gc_prev & 0x1) != 0
    }

    pub fn set_unreachable(&mut self) {
        self._gc_next |= 0x1;
    }

    pub fn clear_unreachable(&mut self) {
        self._gc_next &= !0x1;
    }

    pub fn is_unreachable(&self) -> bool {
        (self._gc_next & 0x1) != 0
    }

    pub fn is_tracked(&self) -> bool {
        self._gc_next != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<PyObject>),
    Dict(Vec<(PyObject, PyObject)>),
    Custom(*mut c_void),
    None,
}

unsafe impl Send for ObjectData {}
unsafe impl Sync for ObjectData {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PyObject {
    pub gc_head: PyGCHead,
    pub name: String,
    pub data: ObjectData,
    pub refcount: usize,
    pub gc_tracked: bool,
    pub has_finalizer: bool,
    pub id: ObjectId,
}

unsafe impl Send for PyObject {}
unsafe impl Sync for PyObject {}

impl PyObject {
    pub fn new(name: String, data: ObjectData) -> Self {
        Self {
            gc_head: PyGCHead::new(),
            name,
            data,
            refcount: 1,
            gc_tracked: false,
            has_finalizer: false,
            id: ObjectId::new(),
        }
    }

    pub fn new_ffi(name: &str, data: ObjectData, _ptr: *mut c_void) -> Self {
        Self {
            gc_head: PyGCHead::new(),
            name: name.to_string(),
            data,
            refcount: 1,
            gc_tracked: false,
            has_finalizer: false,
            id: ObjectId::new(),
        }
    }

    pub fn new_with_finalizer(name: String, data: ObjectData) -> Self {
        Self {
            gc_head: PyGCHead::new(),
            name,
            data,
            refcount: 1,
            gc_tracked: false,
            has_finalizer: true,
            id: ObjectId::new(),
        }
    }

    pub fn get_refcount(&self) -> usize {
        self.refcount
    }

    pub fn set_refcount(&mut self, count: usize) {
        self.refcount = count;
    }

    pub fn inc_ref(&mut self) {
        self.refcount += 1;
    }

    pub fn dec_ref(&mut self) -> bool {
        if self.refcount > 0 {
            self.refcount -= 1;
            self.refcount == 0
        } else {
            false
        }
    }

    pub fn set_finalizer(&mut self, has_finalizer: bool) {
        self.has_finalizer = has_finalizer;
    }

    pub fn has_finalizer(&self) -> bool {
        self.has_finalizer
    }
}

impl Hash for PyObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for PyObject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PyObject {}

pub struct PyObjectPtr {
    ptr: *mut PyObject,
}

impl PyObjectPtr {
    /// Create a new PyObjectPtr from a raw pointer
    ///
    /// # Safety
    ///
    /// - `ptr` must be a valid pointer to a PyObject
    /// - The pointer must remain valid for the lifetime of the PyObjectPtr
    /// - The caller is responsible for ensuring the pointer is not used after the PyObject is dropped
    pub unsafe fn new(ptr: *mut PyObject) -> Self {
        Self { ptr }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Get a reference to the PyObject
    ///
    /// # Safety
    ///
    /// - The underlying pointer must be valid and point to a PyObject
    /// - The PyObject must not be modified while this reference exists
    /// - The caller must ensure the PyObject remains valid for the lifetime of the reference
    pub unsafe fn as_ref(&self) -> Option<&PyObject> {
        if self.ptr.is_null() {
            None
        } else {
            Some(unsafe { &*self.ptr })
        }
    }

    /// Get a mutable reference to the PyObject
    ///
    /// # Safety
    ///
    /// - The underlying pointer must be valid and point to a PyObject
    /// - The PyObject must not be accessed by other code while this mutable reference exists
    /// - The caller must ensure the PyObject remains valid for the lifetime of the reference
    pub unsafe fn as_mut(&mut self) -> Option<&mut PyObject> {
        if self.ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *self.ptr })
        }
    }
}

impl Drop for PyObjectPtr {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(self.ptr);
            }
        }
    }
}
