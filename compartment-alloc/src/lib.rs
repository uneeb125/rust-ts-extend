use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct CompartmentTag(u32);

impl CompartmentTag {
    pub const NONE: CompartmentTag = CompartmentTag(0);

    pub fn from_id(id: u32) -> Self {
        CompartmentTag(id)
    }

    pub fn id(self) -> u32 {
        self.0
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

thread_local! {
    pub static CURRENT_COMPARTMENT: Cell<u32> = const { Cell::new(0) };
    pub static LAST_CHECKED_TAG: Cell<u32> = const { Cell::new(0) };
}

pub fn set_current_compartment(tag: u32) {
    CURRENT_COMPARTMENT.with(|c| c.set(tag));
}

pub fn current_compartment() -> u32 {
    CURRENT_COMPARTMENT.with(|c| c.get())
}

pub fn set_last_checked_tag(tag: u32) {
    LAST_CHECKED_TAG.with(|c| c.set(tag));
}

pub fn last_checked_tag() -> u32 {
    LAST_CHECKED_TAG.with(|c| c.get())
}

#[lang = "compartment_read_tls"]
#[no_mangle]
pub extern "Rust" fn __compartment_read_tls() -> u32 {
    current_compartment()
}

#[lang = "compartment_set_tls"]
#[no_mangle]
pub extern "Rust" fn __compartment_set_tls(tag: u32) {
    set_current_compartment(tag);
}

const HEADER_SIZE: usize = 8;

pub struct CompartmentAllocator<A: GlobalAlloc = System> {
    pub inner: A,
}

impl CompartmentAllocator<System> {
    #[must_use]
    pub const fn new() -> Self {
        CompartmentAllocator { inner: System }
    }
}

impl<A: GlobalAlloc> CompartmentAllocator<A> {
    #[must_use]
    pub const fn with_inner(inner: A) -> Self {
        CompartmentAllocator { inner }
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CompartmentAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let total_size = layout
            .size()
            .checked_add(HEADER_SIZE)
            .expect("allocation size overflow");
        let total_align = layout.align().max(8);
        let total_layout = Layout::from_size_align(total_size, total_align)
            .expect("invalid layout");

        let ptr = self.inner.alloc(total_layout);
        if ptr.is_null() {
            return ptr;
        }

        let tag = current_compartment();

        ptr.cast::<u32>().write(tag);
        ptr.add(4).cast::<u32>().write(layout.size() as u32);

        ptr.add(HEADER_SIZE)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            std::ptr::write_bytes(ptr, 0, layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let real_ptr = ptr.sub(HEADER_SIZE);
        let tag = real_ptr.cast::<u32>().read();
        let stored_size = real_ptr.add(4).cast::<u32>().read();

        let cur = current_compartment();
        if tag != 0 && cur != 0 && tag != cur {
            std::process::abort();
        }

        let total_size = stored_size as usize + HEADER_SIZE;
        let total_align = layout.align().max(8);
        let total_layout =
            Layout::from_size_align(total_size, total_align).unwrap();

        self.inner.dealloc(real_ptr, total_layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let real_ptr = ptr.sub(HEADER_SIZE);
        let tag = real_ptr.cast::<u32>().read();
        let stored_size = real_ptr.add(4).cast::<u32>().read();

        let cur = current_compartment();
        if tag != 0 && cur != 0 && tag != cur {
            std::process::abort();
        }

        if new_size <= layout.size() && new_size >= layout.size() / 2 {
            real_ptr.add(4).cast::<u32>().write(new_size as u32);
            return ptr;
        }

        let new_layout =
            Layout::from_size_align(new_size, layout.align()).unwrap();
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return std::ptr::null_mut();
        }

        let copy_size = std::cmp::min(layout.size(), new_size);
        std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);

        let total_size = stored_size as usize + HEADER_SIZE;
        let total_align = layout.align().max(8);
        let total_layout =
            Layout::from_size_align(total_size, total_align).unwrap();
        self.inner.dealloc(real_ptr, total_layout);

        new_ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static GLOBAL: CompartmentAllocator<System> = CompartmentAllocator::new();

    #[test]
    fn alloc_dealloc_same_compartment() {
        set_current_compartment(1);
        let layout = Layout::new::<[u8; 32]>();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }

    #[test]
    fn alloc_zeroed_writes_tag() {
        set_current_compartment(0xDEAD);
        let layout = Layout::new::<[u8; 16]>();
        let ptr = unsafe { GLOBAL.alloc_zeroed(layout) };
        assert!(!ptr.is_null());
        let real_ptr = unsafe { ptr.sub(HEADER_SIZE) };
        let tag = unsafe { real_ptr.cast::<u32>().read() };
        assert_eq!(tag, 0xDEAD);
        let user_data = unsafe { std::slice::from_raw_parts(ptr, 16) };
        assert!(user_data.iter().all(|&b| b == 0));
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }

    #[test]
    fn dealloc_untagged_allowed_from_anywhere() {
        set_current_compartment(0);
        let layout = Layout::new::<[u8; 16]>();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        set_current_compartment(99);
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }

    #[test]
    fn dealloc_tag_0_allows_cross_compartment() {
        set_current_compartment(0);
        let layout = Layout::new::<[u8; 16]>();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        set_current_compartment(42);
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }

    #[test]
    fn realloc_preserves_tag() {
        set_current_compartment(1);
        let layout = Layout::new::<[u8; 16]>();
        let ptr = unsafe { GLOBAL.alloc(layout) };

        set_current_compartment(1);
        let new_ptr = unsafe { GLOBAL.realloc(ptr, layout, 32) };
        assert!(!new_ptr.is_null());
        let real_ptr = unsafe { new_ptr.sub(HEADER_SIZE) };
        let tag = unsafe { real_ptr.cast::<u32>().read() };
        assert_eq!(tag, 1);
        let stored_size = unsafe { real_ptr.add(4).cast::<u32>().read() };
        assert_eq!(stored_size, 32);
        unsafe { GLOBAL.dealloc(new_ptr, Layout::new::<[u8; 32]>()) };
    }

    #[test]
    fn realloc_shrink_updates_size() {
        set_current_compartment(1);
        let layout = Layout::new::<[u8; 64]>();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        let new_ptr = unsafe { GLOBAL.realloc(ptr, layout, 32) };
        assert_eq!(ptr, new_ptr);
        let real_ptr = unsafe { new_ptr.sub(HEADER_SIZE) };
        let stored_size = unsafe { real_ptr.add(4).cast::<u32>().read() };
        assert_eq!(stored_size, 32);
        unsafe { GLOBAL.dealloc(new_ptr, Layout::new::<[u8; 32]>()) };
    }

    #[test]
    fn zero_sized_alloc() {
        set_current_compartment(1);
        let layout = Layout::new::<()>();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        let real_ptr = unsafe { ptr.sub(HEADER_SIZE) };
        let tag = unsafe { real_ptr.cast::<u32>().read() };
        assert_eq!(tag, 1);
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}
