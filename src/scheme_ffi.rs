//! The small libuim Scheme ABI used by the native plugin.
//!
//! Scheme values held in Rust heap containers must be rooted explicitly:
//! SigScheme's conservative stack scan cannot see a Vec's allocation.

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};

pub type Value = *mut c_void;

unsafe extern "C" {
    pub fn uim_scm_init_proc0(name: *const c_char, callback: extern "C" fn() -> Value);
    pub fn uim_scm_init_proc1(name: *const c_char, callback: extern "C" fn(Value) -> Value);
    pub fn uim_scm_init_proc2(name: *const c_char, callback: extern "C" fn(Value, Value) -> Value);
    pub fn uim_scm_init_proc4(
        name: *const c_char,
        callback: extern "C" fn(Value, Value, Value, Value) -> Value,
    );
    fn uim_scm_gc_protect(location: *mut Value);
    fn uim_scm_gc_unprotect(location: *mut Value);
    fn uim_scm_make_str(text: *const c_char) -> Value;
    fn uim_scm_make_symbol(text: *const c_char) -> Value;
    fn uim_scm_make_int(number: c_long) -> Value;
    fn uim_scm_make_bool(value: c_long) -> Value;
    pub fn uim_scm_c_int(value: Value) -> c_long;
    pub fn uim_scm_c_bool(value: Value) -> c_long;
    pub fn uim_scm_intp(value: Value) -> c_int;
    pub fn uim_scm_refer_c_str(value: Value) -> *const c_char;
    pub fn uim_scm_c_ptr(value: Value) -> *mut c_void;
    pub fn uim_scm_make_ptr(value: *mut c_void) -> Value;
    pub fn uim_scm_nullify_c_ptr(value: Value);
    fn uim_scm_null() -> Value;
    fn uim_scm_cons(head: Value, tail: Value) -> Value;
}

pub struct Root(Box<Value>);

impl Root {
    fn new(value: Value) -> Self {
        let mut root = Self(Box::new(value));
        unsafe { uim_scm_gc_protect(&mut *root.0) };
        root
    }

    pub fn value(&self) -> Value {
        *self.0
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        unsafe { uim_scm_gc_unprotect(&mut *self.0) };
    }
}

pub fn text(value: &str) -> Root {
    let value = CString::new(value).expect("UIM text must not contain NUL");
    Root::new(unsafe { uim_scm_make_str(value.as_ptr()) })
}

pub fn symbol(value: &CStr) -> Root {
    Root::new(unsafe { uim_scm_make_symbol(value.as_ptr()) })
}

pub fn integer(value: i64) -> Root {
    Root::new(unsafe { uim_scm_make_int(value as c_long) })
}

pub fn boolean(value: bool) -> Root {
    Root::new(unsafe { uim_scm_make_bool(value.into()) })
}

pub fn pair(head: &Root, tail: &Root) -> Root {
    Root::new(unsafe { uim_scm_cons(head.value(), tail.value()) })
}

pub fn list<I>(values: I) -> Root
where
    I: IntoIterator<Item = Root>,
    I::IntoIter: DoubleEndedIterator,
{
    let mut result = Root::new(unsafe { uim_scm_null() });
    for value in values.into_iter().rev() {
        // Both cons operands stay protected during allocation. Update the
        // existing stable root before dropping the element's own root.
        *result.0 = unsafe { uim_scm_cons(value.value(), result.value()) };
    }
    result
}
