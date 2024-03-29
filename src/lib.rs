use std::fmt::Debug;

pub trait Dyn {}
impl<T: ?Sized> Dyn for T {}

struct DebugFunctionPtr(*mut ());

impl Debug for DebugFunctionPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("FunctionPtr");
        debug_struct.field("ptr", &self.0);
        let mut symbol_found = false;
        // We need to add one as the IP always points to the next instruction
        backtrace::resolve(unsafe { self.0.byte_add(1) } as *mut _, |symbol| {
            if symbol_found {
                return;
            }
            symbol_found = true;
            if let Some(addr) = symbol.addr() {
                debug_struct.field("relative_addr", &addr);
            }
            if let Some(name) = symbol.name() {
                debug_struct.field("name", &name);
            }
            if let Some(filename) = symbol.filename() {
                debug_struct.field("filename", &filename);
            }
            if let Some(lineno) = symbol.lineno() {
                debug_struct.field("lineno", &lineno);
            }
            if let Some(colno) = symbol.colno() {
                debug_struct.field("colno", &colno);
            }
        });
        debug_struct.finish()
    }
}

#[repr(C)]
pub struct VTable<const N: usize> {
    pub drop_in_place: unsafe fn(*mut ()),
    pub size: usize,
    pub align: usize,
    pub functions: [*mut (); N],
}

impl<const N: usize> Debug for VTable<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VTable")
            .field(
                "drop_in_place",
                &DebugFunctionPtr(self.drop_in_place as *mut ()),
            )
            .field("size", &self.size)
            .field("align", &self.align)
            .field("functions", &self.functions.map(DebugFunctionPtr))
            .finish()
    }
}

#[macro_export]
macro_rules! read_vtable {
    ($trait:path, $n:expr, $v:expr) => {{
        unsafe fn read_vtable(v: &dyn $trait) -> &'static $crate::VTable<$n> {
            let ptr = v as *const dyn $trait;
            let fat_ptr = unsafe { std::mem::transmute::<_, [*const (); 2]>(ptr) };
            unsafe { &*(fat_ptr[1] as *const $crate::VTable<$n>) }
        }
        read_vtable($v)
    }};
}

/// # Safety
/// Callers must ensure that `v` is not dropped after calling this.
/// This can be achieved by using `std::mem::forget`.
pub unsafe fn drop_using_vtable_mut_ref<T: Dyn>(v: &mut T) {
    let vtable = read_vtable!(Dyn, 0, v);
    let dyn_ptr = v as *const dyn Dyn;
    unsafe { (vtable.drop_in_place)(dyn_ptr as *mut ()) };
}

pub fn drop_using_vtable<T: Dyn>(mut v: T) {
    unsafe {
        drop_using_vtable_mut_ref(&mut v);
    }
    std::mem::forget(v);
}

#[cfg(test)]
mod test {
    use std::{fmt::Debug, rc::Rc};

    use super::*;

    fn test_vtable_size_and_align<T: Dyn>(v: &T) {
        let vtable = unsafe { read_vtable!(crate::Dyn, 0, v) };
        assert_eq!(vtable.size, std::mem::size_of::<T>());
        assert_eq!(vtable.align, std::mem::align_of::<T>());
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    struct Test(u16, u8);

    #[derive(Default)]
    #[repr(packed(1))]
    struct Align1 {
        _a: u64,
    }
    #[derive(Default)]
    #[repr(packed(2))]
    struct Align2 {
        _a: u64,
    }
    #[derive(Default)]
    #[repr(packed(4))]
    struct Align4 {
        _a: u64,
    }

    #[test]
    fn vtable_sizes() {
        test_vtable_size_and_align(&0u8);
        test_vtable_size_and_align(&0u16);
        test_vtable_size_and_align(&0u32);
        test_vtable_size_and_align(&0u64);
        test_vtable_size_and_align(&"foo".to_string());
        test_vtable_size_and_align(&Test(1, 2));

        assert_eq!(std::mem::align_of::<Align1>(), 1);
        assert_eq!(std::mem::align_of::<Align2>(), 2);
        assert_eq!(std::mem::align_of::<Align4>(), 4);
        test_vtable_size_and_align(&Align1::default());
        test_vtable_size_and_align(&Align2::default());
        test_vtable_size_and_align(&Align4::default());
    }

    trait TestTrait {
        fn as_ptr(&self) -> *const ();
    }

    impl<T: Debug> TestTrait for T {
        fn as_ptr(&self) -> *const () {
            self as *const T as _
        }
    }

    #[test]
    fn test_virutal_function() {
        let test = Test(3, 4);
        let vtable = unsafe { read_vtable!(TestTrait, 1, &test) };
        dbg!(vtable);

        let as_ptr_f = vtable.functions[0];
        let f = unsafe { std::mem::transmute::<_, fn(&Test) -> *const ()>(as_ptr_f) };

        let ptr = f(&test);
        assert_eq!(ptr, &test as *const Test as *const _);
    }

    #[test]
    fn drop() {
        let v = Rc::new(0);
        let weak = Rc::downgrade(&v);
        assert!(weak.upgrade().is_some());
        drop_using_vtable(v);
        assert!(weak.upgrade().is_none());
    }

    trait ManyFunctions {
        fn c(&self) -> &str {
            "c"
        }
        fn e(&self) -> &str {
            "e"
        }
        fn a(&self) -> &str {
            "a"
        }
        fn b(&self) -> &str {
            "b"
        }
        fn d(&self) -> &str {
            "d"
        }
    }

    impl ManyFunctions for Test {}

    #[test]
    fn test_multiple_virtual_functions() {
        let v = Test(1, 2);
        let vtable = unsafe { read_vtable!(ManyFunctions, 5, &v) };
        eprintln!("{:#?}", vtable);

        for (f, expected) in vtable.functions.into_iter().zip(["c", "e", "a", "b", "d"]) {
            eprintln!("{}", expected);
            let f = unsafe { std::mem::transmute::<_, fn(&Test) -> &str>(f) };
            assert_eq!(f(&v), expected);
        }
    }
}
