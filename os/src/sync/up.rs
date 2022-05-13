// use core::cell::{RefCell, RefMut, UnsafeCell};
use core::ops::{Deref, DerefMut};
use riscv::register::sstatus;
use lazy_static::*;
use lock::{Mutex, MutexGuard};


pub struct IntrMaskingInfo {
    nested_level: usize,
    sie_before_masking: bool,
}

lazy_static! {
    static ref INTR_MASKING_INFO: UPSafeCell<IntrMaskingInfo> = unsafe {
        UPSafeCell::new(IntrMaskingInfo::new()) 
    };
}

impl IntrMaskingInfo {
    pub fn new() -> Self {
        Self {
            nested_level: 0,
            sie_before_masking: false,
        }
    }

    pub fn enter(&mut self) {
        let sie = sstatus::read().sie();
        unsafe { sstatus::clear_sie(); }
        if self.nested_level == 0 {
            self.sie_before_masking = sie;
        }
        self.nested_level += 1;
    }

    pub fn exit(&mut self) {
        self.nested_level -= 1;
        if self.nested_level == 0 && self.sie_before_masking {
            unsafe { sstatus::set_sie(); }            
        }
    }
}

pub struct UPIntrFreeCell<T> {
    /// inner data
    inner: lock::Mutex<T>,
}


unsafe impl<T> Sync for UPIntrFreeCell<T> {}

pub struct UPIntrRefMut<'a, T>(Option<MutexGuard<'a, T>>);

impl<T> UPIntrFreeCell<T> {
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: lock::Mutex::new(value),
        }
    }
    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> MutexGuard<T> {
        INTR_MASKING_INFO.exclusive_access().enter();
        self.inner.lock()
    }

    pub fn exclusive_session<F, V>(&self, f: F) -> V where F: FnOnce(&mut T) -> V {
        let mut inner = self.exclusive_access();
        f(inner.deref_mut())
    }
}

impl<'a, T> Drop for UPIntrRefMut<'a, T> {
    fn drop(&mut self) {
        self.0 = None;
        INTR_MASKING_INFO.exclusive_access().exit();
    }
}

impl<'a, T> Deref for UPIntrRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap().deref()
    }
}
impl<'a, T> DerefMut for UPIntrRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap().deref_mut()
    }
}


pub struct UPSafeCell<T> {
    /// inner data
    inner: Mutex<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    /// User is responsible to guarantee that inner struct is only used in
    /// uniprocessor.
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }
    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> MutexGuard<T> {
        self.inner.lock()
    }
}