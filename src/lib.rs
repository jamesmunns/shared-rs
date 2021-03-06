//! # `shared`
//!
//! A moderately low cost, easy to use, safe abstraction for sharing
//! data between application and interrupt context.
//!
//! ## Example
//!
//! ```rust
//! use nrf52832_pac::Interrupt;
//! use bare_metal;
//! use cortex_m;
//!
//! // Tuples are of the format:
//! //  (VARIABLE_NAME, VARIABLE_TYPE, CORRESPONDING_INTERRUPT),
//! shared!(
//!     (RADIO_PKTS, usize, Interrupt::RADIO),
//!     (WALL_CLOCK, usize, Interrupt::RTC0),
//! );
//!
//! #[entry]
//! fn main() {
//!     // Using a `shared` data item in non-interrupt context
//!     // requires a token. This is a singleton, sort of like
//!     // the peripherals from a peripheral access crate
//!     let mut token = RADIO_PKTS::set_initial(27).unwrap();
//!
//!     // You access the data from within a closure. The interrupt
//!     // this data is shared with is disabled for the duration of
//!     // the closure. Other interrupts may still occur.
//!     token.modify_app_context(|y| {
//!         *y -= 1;
//!         y
//!     }).unwrap();
//! }
//!
//! #[interrupt]
//! fn RADIO() {
//!     // Within an interrupt, access is only granted if it matches
//!     // the declared interrupt. Inside the `RADIO` interrupt here,
//!     // only `RADIO_PKTS` is accessible.
//!     //
//!     // Access from within an interrupt doesn't require a token.
//!     RADIO_PKTS::modify_int_context(|x| {
//!         *x += 1;
//!         x
//!     }).unwrap();
//! }
//!
//! #[interrupt]
//! fn RTC0() {
//!     // If `set_initial` was never called, then all attempts to
//!     // access will return an `Err`. This code would panic at
//!     // runtime!
//!     BAZ::modify_int_context(|x| {
//!         *x += 1;
//!         x
//!     }).unwrap();
//! }
//! ```

#![no_std]

#[macro_export]
macro_rules! shared {
    (
        $(($NAME:ident, $dat_ty:ty, $int:expr),)+
    ) => {
        /// Re-export all the structures at the top level, making them
        /// visible at the scope the macro was used (not necessarily global!)
        pub use shared_internals::structs::*;

        /// This module is basically just here to hide all of the stuff
        /// from being public
        #[doc(hidden)]
        pub mod shared_internals {

            /// These are the actual data structures that back the
            /// shared data
            mod singletons {
                $(
                    pub static mut $NAME: Option<$dat_ty> = None;
                )+
            }

            /// These flags are used to prevent re-entrant calls from within
            /// an interrupt
            mod flags {
                use ::core::sync::atomic::AtomicBool;
                $(
                    pub static $NAME: AtomicBool = AtomicBool::new(false);
                )+
            }

            /// This is the primary interface to the shared data. The struct itself
            /// is actually an opaque zero sized type, with methods that grab data
            /// from the `flags` and `singletons` modules
            pub mod structs {
                use ::core::sync::atomic::Ordering;
                use ::cortex_m::peripheral::NVIC;
                use ::bare_metal::Nr;

                // This is bad. I don't know how else to generically get
                // the interrupt enum provided by the -PAC though.
                // PRs welcome :)
                use super::super::Interrupt;

                $(
                    pub struct $NAME {
                        _private: ()
                    }

                    impl $NAME {
                        /// Set the initial value of the shared data. This must be done
                        /// from application context, not interrupt context.
                        ///
                        /// This function must be called before the `modify_*` methods
                        /// can be used, otherwise they will return errors.
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

                        /// Access the shared data from the application (non-interrupt) context.
                        /// The interrupt must not be active when calling this function.
                        ///
                        /// During the scope of the closure, the corresponding interrupt will be
                        /// disabled to prevent concurrent access.
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

                        /// Access the shared data from the interrupt context. This function will
                        /// only work if the corresponding interrupt is currently active. This
                        /// function is not re-entrant - you cannot grab the shared data more than
                        /// once.
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

                /////////////////////////////////////////////////////////
                // This section comes from the cortex-m crate.
                //
                // Ideally, we wouldn't need to copy/paste code, but
                // I don't think it's possible to use these functions without
                // having a mutable reference to the NVIC, which would require
                // something taking ownership of it.
                //
                // PRs welcome if this could be done better!
                /////////////////////////////////////////////////////////

                /// This method comes from `cortex-m::NVIC`
                fn int_is_enabled<I>(interrupt: I) -> bool
                    where I: Nr,
                {
                    let nr = interrupt.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iser[usize::from(nr / 32)].read() & mask) == mask }
                }

                /// This method comes from `cortex-m::NVIC`
                fn int_is_active<I>(interrupt: I) -> bool
                    where I: Nr
                {
                    let nr = interrupt.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iabr[usize::from(nr / 32)].read() & mask) == mask }
                }

                /// This method comes from `cortex-m::NVIC`
                fn disable_int<I>(interrupt: I)
                    where I: Nr
                {
                    let nr = interrupt.nr();

                    unsafe { (*NVIC::ptr()).icer[usize::from(nr / 32)].write(1 << (nr % 32)) }
                }

                /// This method comes from `cortex-m::NVIC`
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
