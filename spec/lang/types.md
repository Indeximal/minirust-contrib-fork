# MiniRust types

This file defines the types of MiniRust.
Note that MiniRust types play a somewhat different role than Rust types:
every Rust type corresponds to a MiniRust type, but MiniRust types mostly just serve to define how [values](values.md) are represented in memory.
Basically, they define a (de)serialization format -- the **representation relation**, defined by an "encode" function to turn values into byte lists, and a "decode" function for the opposite operation.
In particular, MiniRust is by design *not type-safe*.
However, the representation relation is a key part of the language, since it forms the interface between the low-level and high-level view of data, between lists of (abstract) bytes and values.

That said, types do have a little more information than required for the representation relation.
For pointer types (references and raw pointers), types also contain a "mutability", which does not affect the representation relation but can be relevant for the aliasing rules.
(We might want to organize this differently in the future, and remove mutability from types.)
Union types know the types of their fields solely to support union field place projections.

Note that for now, we make the exact offsets of each field part of the type.
As always, this definition is incomplete.
In the future, we might want to separate a type from its layout, and consider these separate components -- we will have to see what works best.

```rust
/// The types of MiniRust.
pub enum Type {
    Int(IntType),
    Bool,
    /// `Ptr` represents all pointer types: references, raw pointers, boxes, and function pointers.
    /// A pointer type does *not* need the full pointee type, since (de)serializing a pointer does not
    /// require knowledge about the pointee. We only track basic pointee information like size and
    /// alignment that is required to check reference validity. This also means types have a finite
    /// representation even when the Rust type is recursive.
    Ptr(PtrType),
    /// "Tuple" is used for all heterogeneous types, i.e., both Rust tuples and structs.
    Tuple {
        /// Fields must not overlap.
        fields: Fields,
        /// The total size of the tuple can indicate trailing padding.
        /// Must be large enough to contain all fields.
        size: Size,
        /// Total alignment of the tuple. Due to `repr(packed)` and `repr(align)`,
        /// this is independent of the fields' alignment.
        align: Align,
    },
    Array {
        #[specr::indirection]
        elem: Type,
        count: Int,
        // TODO: store whether this is a (SIMD) vector, and something about alignment?
    },
    /// Slices, i.e. `[T]` are unsized types which therefore cannot be represented as values.
    Slice {
        #[specr::indirection]
        elem: Type,
    },
    Union {
        /// Fields *may* overlap. Fields only exist for field access place projections,
        /// they are irrelevant for the representation relation.
        fields: Fields,
        /// A union can be split into multiple "chunks", where only the data inside those chunks is
        /// preserved, and data between chunks is lost (like padding in a struct).
        /// This is necessary to model the behavior of some `repr(C)` unions, see
        /// <https://github.com/rust-lang/unsafe-code-guidelines/issues/156> for details.
        chunks: List<(Offset, Size)>,
        /// The total size of the union, can indicate padding after the last chunk.
        size: Size,
        /// Total alignment of the union. Due to `repr(packed)` and `repr(align)`,
        /// this is independent of the fields' alignment.
        align: Align,
    },
    Enum {
        /// The map variants, each identified by a discriminant. Each variant is given by a type and its
        /// tag description. All variants are thought to "start at offset 0"; if the
        /// discriminant is encoded as an explicit tag, then that will be put into the
        /// padding of the active variant. (This means it is *not* safe to hand out mutable
        /// references to a variant at that type, as then the tag might be overwritten!)
        /// The Rust type `!` is encoded as an `Enum` with an empty list of variants.
        variants: Map<Int, Variant>,
        /// The `IntType` for the discriminant. This is used for the type of
        /// `GetDiscriminant` and `SetDiscriminant`. It is entirely independent of how
        /// the discriminant is represented in memory (the "tag").
        discriminant_ty: IntType,
        /// The decision tree to decode the discriminant from the tag at runtime.
        discriminator: Discriminator,
        /// The total size of the enum can indicate trailing padding.
        /// Must be large enough to contain all variants.
        size: Size,
        /// Total alignment of the enum. Due to `repr(packed)` and `repr(align)`,
        /// this is independent of the fields' alignment.
        align: Align,
    },
    /// This is surprisingly not used for method invocation I think, as only the pointer metadata counts.
    /// I actually think this is not used at all in MiniRust. Since an `dyn Foo` Place can do nothing, not assigning to it, 
    /// not field or index projection.
    /// It might be used in minimize. ??
    TraitObject,
}

pub struct IntType {
    pub signed: Signedness,
    pub size: Size,
}

pub type Fields = List<(Offset, Type)>;

pub struct Variant {
    /// The actual type of the variant.
    pub ty: Type,
    /// The information on where to store which values to write the tag.
    /// MUST NOT touch any bytes written by the actual type of the variant and vice
    /// versa. This is because we allow references/pointers to (enum) fields which
    /// should be able to dereference without having to deal with the tag.
    pub tagger: Map<Offset, (IntType, Int)>,
}

/// The decision tree that computes the discriminant out of the tag for a specific
/// enum type.
pub enum Discriminator {
    /// We know the discriminant.
    Known(Int),
    /// Tag decoding failed, there is no valid discriminant.
    Invalid,
    /// We don't know the discriminant, so we branch on the value of a specific value.
    Branch {
        offset: Offset,
        value_type: IntType,
        #[specr::indirection]
        fallback: Discriminator,
        /// An left-inclusive right-exclusive range of values that map to some Discriminator.
        children: Map<(Int, Int), Discriminator>,
    },
}
```

Note that references have no lifetime, since the lifetime is irrelevant for their representation in memory!
They *do* have a mutability since that is (or will be) relevant for the memory model.

## Layout of a type

Here we define the size and other layout properties of a type.

```rust
impl IntType {
    pub fn align<T: Target>(self) -> Align {
        let size = self.size.bytes();
        // The size is a power of two, so we can use it as alignment.
        let natural_align = Align::from_bytes(size).unwrap();
        // Integer alignment is capped by the target.
        natural_align.min(T::INT_MAX_ALIGN)
    }
}

impl Type {
    pub fn size<T: Target>(self) -> SizeStrategy {
        use Type::*;
        use SizeStrategy::Sized;
        match self {
            Int(int_type) => Sized(int_type.size),
            Bool => Sized(Size::from_bytes_const(1)),
            Ptr(p) if p.meta_kind() == PointerMetaKind::None => Sized(T::PTR_SIZE),
            Ptr(_) => Sized(libspecr::Int::from(2) * T::PTR_SIZE),
            Tuple { size, .. } | Union { size, .. } | Enum { size, .. } => Sized(size),
            Array { elem, count } => Sized(
                elem.size::<T>().expect_sized("WF ensures array element is sized") * count
            ),
            Slice { elem } => SizeStrategy::Slice(elem.size::<T>().expect_sized("WF ensures slice element is sized")),
            TraitObject => SizeStrategy::TraitObject,
        }
    }

    pub fn align<T: Target>(self) -> Align {
        use Type::*;
        match self {
            Int(int_type) => int_type.align::<T>(),
            Bool => Align::ONE,
            Ptr(_) => T::PTR_ALIGN,
            Tuple { align, .. } | Union { align, .. } | Enum { align, .. } => align,
            Array { elem, .. } | Slice { elem } => elem.align::<T>(),
            // FIXME: This is wrong, need to be encode in AlignStrategy.
            TraitObject => T::PTR_ALIGN,
        }
    }

    /// Returns the metadata kind when this type is used as a pointee.
    pub fn meta_kind(self) -> PointerMetaKind {
        match self {
            Type::Slice { .. } => PointerMetaKind::ElementCount,
            Type::TraitObject => PointerMetaKind::VTablePointer,
            _ => PointerMetaKind::None
        }
    }
}
```

And we also define how to compute the actual size, based on additional state.

```rust
impl SizeStrategy {
    pub fn is_sized(self) -> bool {
        matches!(self, SizeStrategy::Sized(_))
    }

    /// Returns the size when the type must be statically sized.
    pub fn expect_sized(self, msg: &str) -> Size {
        match self {
            SizeStrategy::Sized(size) => size,
            _ => panic!("expect_sized called on unsized type: {msg}"),
        }
    }

    /// Computes the dynamic size, but the caller must provide compatible metadata.
    pub fn compute(self, meta: Option<PointerMeta>, vtables: Map<VTableName, VTable>) -> Result<Size> {
        match (self, meta) {
            (SizeStrategy::Sized(size), None) => ret(size),
            (SizeStrategy::Slice(elem_size), Some(PointerMeta::ElementCount(count))) => {
                let raw_size = count * elem_size;
                // FIXME(UnsizedTypes): We need to assert that the resulting size isn't too big.
                ret(raw_size)
            }
            (SizeStrategy::TraitObject, Some(PointerMeta::VTablePointer(vtable_name))) => {
                let Some(vtable) = vtables.get(vtable_name) else {
                    throw_ub!("Computing the size of a trait object with invalid vtable in pointer");
                };
                ret(vtable.size)
            }
            _ => panic!("pointer meta data does not match type"),
        }
    }

    /// Returns the metadata kind which is needed to compute this strategy,
    /// i.e `self.meta_kind().matches(meta)` implies `self.compute(meta, _)` does not panic.
    pub fn meta_kind(self) -> PointerMetaKind {
        match self {
            SizeStrategy::Sized(_) => PointerMetaKind::None,
            SizeStrategy::Slice(_) => PointerMetaKind::ElementCount,
            SizeStrategy::TraitObject => PointerMetaKind::VTablePointer,
        }
    }
}
```

## Integer type convenience functions

```rust
impl IntType {
    pub const I8: IntType = IntType { signed: Signedness::Signed, size: Size::from_bytes_const(1) };

    pub fn can_represent(&self, i: Int) -> bool {
        i.in_bounds(self.signed, self.size)
    }

    pub fn bring_in_bounds(&self, i: Int) -> Int {
        i.bring_in_bounds(self.signed, self.size)
    }

    /// Generate the return type for IntWithOverflow
    pub fn with_overflow<T: Target>(&self) -> Type {
        // Define a tuple type with two fields: An integer followed directly by a boolean.
        let fields = list![(Size::ZERO, Type::Int(*self)), (self.size, Type::Bool)];
        // Alignment is always the one of the integer as boolean has align requirement 1.
        let align = self.align::<T>();
        // The total size is `self.size + 1` rounded up to the next multiple of `align`.
        // Since `self.size` is already a multiple of `align`, we can compute this as follows:
        let size = self.size + Size::from_bytes(align.bytes()).unwrap();
        Type::Tuple { fields, size, align }
    }
}
```
