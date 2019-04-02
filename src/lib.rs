#![no_std]

#[macro_export]
macro_rules! shared {
    (
        $(($NAME:ident, $dat_ty:ty, $int:expr),)+
    ) => {
        pub use shared::structs::*;

        pub mod shared {
            mod singletons {
                $(
                    pub static mut $NAME: Option<$dat_ty> = None;
                )+
            }

            mod flags {
                use ::core::sync::atomic::AtomicBool;
                $(
                    pub static $NAME: AtomicBool = AtomicBool::new(false);
                )+
            }

            pub mod structs {
                use ::core::sync::atomic::Ordering;
                use ::cortex_m::peripheral::NVIC;
                use ::bare_metal::Nr;

                // This is bad. I don't know how else to generically get
                // the interrupt enum provided by the -PAC though.
                use super::super::Interrupt;

                $(
                    pub struct $NAME {
                        _private: ()
                    }

                    impl $NAME {
                        pub fn set_initial(data: $dat_ty) -> Result<$NAME, $dat_ty> {
                            if int_is_enabled($int) || super::flags::$NAME.load(Ordering::SeqCst) {
                                return Err(data);
                            }

                            if unsafe { super::singletons::$NAME.is_none() } {
                                unsafe {
                                    super::singletons::$NAME = Some(data);
                                }
                                Ok($NAME { _private: () })
                            } else {
                                Err(data)
                            }
                        }

                        pub fn modify_app_context<F>(&mut self, f: F) -> Result<(), ()>
                        where
                            for<'w> F: FnOnce(&'w mut $dat_ty) -> &'w mut $dat_ty,
                        {
                            // theoretical race condition: if an interrupt enables this interrupt between
                            // the next line and the line after
                            let enabled = int_is_enabled($int);
                            if enabled {
                                disable_int($int);
                            }
                            if int_is_active($int) || unsafe { super::singletons::$NAME.is_none() } {
                                if enabled {
                                    enable_int($int);
                                }
                                return Err(());
                            }

                            unsafe {
                                f(super::singletons::$NAME.as_mut().unwrap());
                            }

                            if enabled {
                                enable_int($int);
                            }

                            Ok(())
                        }

                        pub fn modify_int_context<F>(f: F) -> Result<(), ()>
                        where
                            for<'w> F: FnOnce(&'w mut $dat_ty) -> &'w mut $dat_ty,
                        {
                            if !int_is_active($int) || unsafe { super::singletons::$NAME.is_none() } || super::flags::$NAME.swap(true, Ordering::SeqCst) {
                                return Err(());
                            }

                            unsafe {
                                f(super::singletons::$NAME.as_mut().unwrap());
                            }

                            assert!(super::flags::$NAME.swap(false, Ordering::SeqCst));
                            Ok(())

                        }
                    }
                )+

                fn int_is_enabled<I>(interrupt: I) -> bool
                    where I: Nr,
                {

                    let nr = interrupt.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iser[usize::from(nr / 32)].read() & mask) == mask }
                }

                fn int_is_active<I>(interrupt: I) -> bool
                    where I: Nr
                {
                    let nr = interrupt.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iabr[usize::from(nr / 32)].read() & mask) == mask }
                }

                fn disable_int<I>(interrupt: I)
                    where I: Nr
                {
                    let nr = interrupt.nr();

                    unsafe { (*NVIC::ptr()).icer[usize::from(nr / 32)].write(1 << (nr % 32)) }
                }

                /// Enables `interrupt`
                fn enable_int<I>(interrupt: I)
                    where I: Nr
                {
                    let nr = interrupt.nr();

                    unsafe { (*NVIC::ptr()).iser[usize::from(nr / 32)].write(1 << (nr % 32)) }
                }
            }
        }
    }
}
