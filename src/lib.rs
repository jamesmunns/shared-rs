#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

use bare_metal::Nr;
use cortex_m::peripheral::NVIC;

macro_rules! cmim {
    (
        $(($name:ident, $bool_name:ident, $set_fn:ident, $get_fn:ident, $int:expr, $dat:ty),)+
    ) => {
        pub static $name: Option<$dat> = None;

        pub fn $set_fn(data: $dat) {

        }

    }
}

enum IntStub {
    Foo,
    Bar,
}

unsafe impl Nr for IntStub {
    fn nr(&self) -> u8 {
        match *self {
            IntStub::Foo => 3,
            IntStub::Bar => 4,
        }
    }
}

macro_rules! cmim {
    (
        $(($name:ident, $bool_name:ident, $set_fn:ident, $get_fn:ident, $int:expr, $dat:ty),)+
    ) => {
        pub mod interrupt_data {
            use super::*;
            $(
                static mut $name: Option<$dat> = None;
                static $bool_name: ::core::sync::atomic::AtomicBool = ::core::sync::atomic::AtomicBool::new(false);

                /// Setter function for $name. $int must not be enabled or
                /// currently active.
                pub fn $set_fn(data: $dat) {
                    assert!(!int_is_enabled($int));
                    assert!(!$bool_name.load(::core::sync::atomic::Ordering::SeqCst));

                    unsafe {
                        $name = Some(data);
                    }
                }

                /// Getter function for $name. Not re-entrant. Must only be called from
                /// within the $int interrupt. Gain mutable access to $name for the duration
                /// of a given closure.
                pub fn $get_fn<F>(f: F)
                where
                    for<'w> F: FnOnce(&'w mut $dat) -> &'w mut $dat,
                    {
                        assert!(int_is_active($int));
                        assert!(!$bool_name.swap(true, ::core::sync::atomic::Ordering::SeqCst));

                        unsafe {
                            f($name.as_mut().unwrap());
                        }

                        assert!($bool_name.swap(false, ::core::sync::atomic::Ordering::SeqCst));
                    }
            )+

            fn int_is_enabled<I>(interrupt: I) -> bool
            where
                I: ::bare_metal::Nr,
            {

                let nr = interrupt.nr();
                let mask = 1 << (nr % 32);

                // NOTE(unsafe) atomic read with no side effects
                unsafe { ((*::cortex_m::peripheral::NVIC::ptr()).iser[usize::from(nr / 32)].read() & mask) == mask }
            }

            pub fn int_is_active<I>(interrupt: I) -> bool
            where
                I: ::bare_metal::Nr,
            {
                let nr = interrupt.nr();
                let mask = 1 << (nr % 32);

                // NOTE(unsafe) atomic read with no side effects
                unsafe { ((*::cortex_m::peripheral::NVIC::ptr()).iabr[usize::from(nr / 32)].read() & mask) == mask }
            }
        }
    }
}

cmim!(
    (BAZ, BAZ_FLAG, set_baz, get_baz, IntStub::Foo, usize),
);

fn main() {
    interrupt_data::set_baz(123);

    interrupt_data::get_baz(
        |x| {
            *x += 20;
            x
        }
    );
}
