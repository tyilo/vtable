use std::{pin::Pin, rc::Rc, sync::Arc};

use vtable::read_vtable;

trait TraitMethods {
    fn by_ref(&self) {}
    fn by_ref_mut(&mut self) {}
    fn by_box(self: Box<Self>) {}
    fn by_rc(self: Rc<Self>) {}
    fn by_arc(self: Arc<Self>) {}
    fn by_pin(self: Pin<&Self>) {}
    fn with_lifetime(&'static self) {}
    fn nested_pin(self: Pin<Arc<Self>>) {}
    fn overriden(&self) {}
}

struct Struct();
impl TraitMethods for Struct {
    fn overriden(&self) {}
}

fn main() {
    let v = Struct();
    let vtable = unsafe { read_vtable!(TraitMethods, 9, &v) };
    eprintln!("{:#?}", vtable);
}
