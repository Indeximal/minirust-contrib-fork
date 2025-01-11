/// This struct will be a DST iff T is a DST
struct Foo<T: ?Sized> {
    a: u16,
    c: u8,
    b: T,
}

trait Bar {
    fn get(&self) -> usize;
}
impl Bar for usize {
    fn get(&self) -> usize {
        *self
    }
}

fn main() {
    let f: Foo<usize> = Foo { a: 0, c: 1, b: 11 };
    // usize is coerced to dyn Bar in inner, needs to be behind pointer
    let g: &Foo<dyn Bar> = &f;
    // first project to field and then perform dynamic dispatch
    assert_eq!(g.b.get(), 11);
    // assert that the struct is properly padded to 16 bytes
    assert_eq!(core::mem::size_of_val(g), 16);
}
