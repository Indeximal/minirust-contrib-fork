# MiniRust pointers

One key question a memory model has to answer is *what is a pointer*.
It might seem like the answer is just "an integer of appropriate size", but [that is not the case][pointers-complicated] (as [more][pointers-complicated-2] and [more][pointers-complicated-3] discussion shows).
This becomes even more prominent with aliasing models such as [Stacked Borrows].
The memory model hence takes the stance that a pointer consists of the *address* (which truly is just an integer of appropriate size) and a *provenance*.
What exactly [provenance] *is* is up to the memory model.
As far as the interface is concerned, this is some opaque extra data that we carry around with our pointers and that places restrictions on which pointers may be used to do what when.

[pointers-complicated]: https://www.ralfj.de/blog/2018/07/24/pointers-and-bytes.html
[pointers-complicated-2]: https://www.ralfj.de/blog/2020/12/14/provenance.html
[pointers-complicated-3]: https://www.ralfj.de/blog/2022/04/11/provenance-exposed.html
[provenance]: https://github.com/rust-lang/unsafe-code-guidelines/blob/master/reference/src/glossary.md#pointer-provenance
[Stacked Borrows]: https://github.com/rust-lang/unsafe-code-guidelines/blob/master/wip/stacked-borrows.md

```rust
/// An "address" is a location in memory. This corresponds to the actual
/// location in the real program.
/// We make it a mathematical integer, but of course it is bounded by the size
/// of the address space.
pub type Address = Int;

/// A "data pointer" is an address together with its Provenance.
/// Provenance can be absent; those pointers are
/// invalid for all non-zero-sized accesses.
pub struct DataPointer<Provenance> {
    pub addr: Address,
    pub provenance: Option<Provenance>,
}

// This naming of "data pointer" as the part without metadata is used in the docs at:
// <https://doc.rust-lang.org/std/primitive.pointer.html>.
// An alternative might be calling it `MemPointer`.
// I believe it is more consistent to call the whole thing `Pointer`, rather than just the data part,
// as this is how most of Rusts references are written (ignoring any metadata fields).

/// A "pointer" is the data pointer with optionally some metadata.
/// Corresponds to the rust primitive "pointer", as well as references and boxes.
pub struct Pointer<Provenance> {
    pub data_pointer: DataPointer<Provenance>,
    pub metadata: Option<PointerMeta>
}

impl<Provenance> DataPointer<Provenance> {
    /// Offsets a pointer in bytes using wrapping arithmetic.
    /// This does not check whether the pointer is still in-bounds of its allocation.
    pub fn wrapping_offset<T: Target>(self, offset: Int) -> Self {
        let addr = self.addr + offset;
        let addr = addr.bring_in_bounds(Unsigned, T::PTR_SIZE);
        DataPointer { addr, ..self }
    }
}
```

```rust
// This doesn't make a lot of sense in this file, maybe rather values.md.
// (Similarly the PtrType is only used for retagging in mem.rs, and it felt out of place when reading ?)
enum PointerMeta {
    ElementCount(Size),
    // TODO
    VTable
}
```


We sometimes need information what it is that a pointer points to, this is captured in a "pointer type".

```rust
/// A "layout" describes what we know about data behind a pointer.
pub struct Layout {
    pub size: Size,
    pub align: Align,
    pub inhabited: bool,
}

pub enum PtrType {
    Ref {
        /// Indicates a shared vs mutable reference.
        /// FIXME: also indicate presence of `UnsafeCell`.
        mutbl: Mutability,
        /// We only need to know the layout of the pointee.
        /// (This also means we have a finite representation even when the Rust type is recursive.)
        pointee: Layout,
    },
    Box {
        pointee: Layout,
    },
    Raw,
    FnPtr,
}

impl PtrType {
    /// If this is a safe pointer, return the pointee layout.
    pub fn safe_pointee(self) -> Option<Layout> {
        match self {
            PtrType::Ref { pointee, .. } | PtrType::Box { pointee, .. } => Some(pointee),
            PtrType::Raw | PtrType::FnPtr => None,
        }
    }

    pub fn addr_valid(self, addr: Address) -> bool {
        if let Some(layout) = self.safe_pointee() {
            // Safe addresses need to be non-null, aligned, and not point to an uninhabited type.
            // (Think: uninhabited types have impossible alignment.)
            addr != 0 && layout.align.is_aligned(addr) && layout.inhabited
        } else {
            true
        }
    }
}
```
