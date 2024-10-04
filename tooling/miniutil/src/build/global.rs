use crate::build::*;

impl ProgramBuilder {
    pub fn declare_global_zero_initialized<T: TypeConv>(&mut self) -> PlaceExpr {
        let bytes = List::from_elem(Some(0), T::get_layout().expect_size("T is `Sized`").bytes());
        let global = Global {
            bytes,
            relocations: list!(),
            align: <T>::get_layout().expect_align("T is `Sized`"),
        };
        let name = GlobalName(Name::from_internal(self.next_global));
        self.next_global += 1;
        self.globals.try_insert(name, global).unwrap();
        global_by_name::<T>(name)
    }
}

/// Global Int initialized to zero.
pub fn global_int<T: TypeConv>() -> Global {
    let bytes = List::from_elem(Some(0), T::get_layout().expect_size("T is `Sized`").bytes());

    Global { bytes, relocations: list!(), align: T::get_layout().expect_align("T is `Sized`") }
}

/// Global pointer
pub fn global_ptr<T: TypeConv + ?Sized>() -> Global {
    let bytes =
        List::from_elem(Some(0), <*const T>::get_layout().expect_size("*T is `Sized`").bytes());

    Global {
        bytes,
        relocations: list!(),
        align: <*const T>::get_layout().expect_align("T is `Sized`"),
    }
}
