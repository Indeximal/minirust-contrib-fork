use crate::*;

#[test]
fn ref_not_ub() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        // declare non boolean
        let no_bool = f.declare_local::<u8>();
        f.storage_live(no_bool);
        f.assign(no_bool, const_int(2_u8));

        // put it behind a boolean reference
        let buul = f.declare_local::<&bool>();
        f.storage_live(buul);

        // This should UB, it doesn't
        f.assign(buul, addr_of(no_bool, <&bool>::get_type()));

        // Using this bool then will.
        f.assume(load(deref(load(buul), <bool>::get_type())));
        f.exit();
        p.finish_function(f)
    };
    let p = p.finish_program(f);
    // Gives "load at type Bool but the data in memory violates the validity invariant".
    assert_ub::<BasicMem>(p, "Reference to invalid type");
}
