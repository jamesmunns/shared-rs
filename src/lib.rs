#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

use bare_metal::Nr;

#[allow(dead_code)]
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
        $(($NAME:ident, $dat_ty:ty, $int:expr),)+
    ) => {
        $(
            #[allow(non_snake_case)]
            pub mod $NAME {
                use super::*;
                use ::core::sync::atomic::Ordering;

                pub struct AppToken {
                    _private: ()
                }

                static mut $NAME: Option<$dat_ty> = None;

                mod flag {
                    use ::core::sync::atomic::AtomicBool;
                    pub static $NAME: AtomicBool = AtomicBool::new(false);
                }

                impl AppToken {
                    pub fn set_initial(data: $dat_ty) -> Result<AppToken, $dat_ty> {
                        if int_is_enabled() || flag::$NAME.load(Ordering::SeqCst) {
                            return Err(data);
                        }

                        if unsafe { $NAME.is_none() } {
                            unsafe {
                                $NAME = Some(data);
                            }
                            Ok(AppToken { _private: () })
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
                        let enabled = int_is_enabled();
                        if enabled {
                            disable_int();
                        }
                        if int_is_active() || unsafe { $NAME.is_none() } {
                            if enabled {
                                enable_int();
                            }
                            return Err(());
                        }

                        unsafe {
                            f($NAME.as_mut().unwrap());
                        }

                        if enabled {
                            enable_int();
                        }

                        Ok(())
                    }

                    pub fn modify_int_context<F>(f: F) -> Result<(), ()>
                    where
                        for<'w> F: FnOnce(&'w mut $dat_ty) -> &'w mut $dat_ty,
                    {
                        if !int_is_active() || unsafe { $NAME.is_none() } || flag::$NAME.swap(true, Ordering::SeqCst) {
                            return Err(());
                        }

                        unsafe {
                            f($NAME.as_mut().unwrap());
                        }

                        assert!(flag::$NAME.swap(false, Ordering::SeqCst));
                        Ok(())

                    }
                }

                use ::cortex_m::peripheral::NVIC;

                fn int_is_enabled() -> bool
                {

                    let nr = $int.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iser[usize::from(nr / 32)].read() & mask) == mask }
                }

                fn int_is_active() -> bool
                {
                    let nr = $int.nr();
                    let mask = 1 << (nr % 32);

                    // NOTE(unsafe) atomic read with no side effects
                    unsafe { ((*NVIC::ptr()).iabr[usize::from(nr / 32)].read() & mask) == mask }
                }

                fn disable_int()
                {
                    let nr = $int.nr();

                    unsafe { (*NVIC::ptr()).icer[usize::from(nr / 32)].write(1 << (nr % 32)) }
                }

                /// Enables `interrupt`
                fn enable_int()
                {
                    let nr = $int.nr();

                    unsafe { (*NVIC::ptr()).iser[usize::from(nr / 32)].write(1 << (nr % 32)) }
                }
            }
        )+
    }
}

cmim!(
    (BAZ, usize, IntStub::Foo),
);

#[allow(dead_code)]
fn main() {
    let mut token = BAZ::AppToken::set_initial(27).unwrap();
    token.modify_app_context(|y| { *y -= 1; y }).unwrap();
}

#[allow(non_snake_case, dead_code)]
fn DEMO_INT() {
    BAZ::AppToken::modify_int_context(|x| { *x += 1; x }).unwrap();
}
