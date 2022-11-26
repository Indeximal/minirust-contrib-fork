This is basically a copy of the `Align` type in the Rust compiler.
See [Align](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_target/abi/struct.Align.html).

`Align` is always a power of two.

```rust
/// `raw` stores the align in bytes.
pub struct Align { raw: Int }

impl Align {
    pub const ONE: Align = Align { raw: Int::ONE };

    /// align is rounded up to the next power of two.
    pub fn from_bytes(align: impl Into<Int>) -> Align {
        let align = align.into();
        let raw = align.next_power_of_two();

        Align { raw }
    }

    pub fn bytes(self) -> Int {
        self.raw
    }
}
```