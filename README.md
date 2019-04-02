# CMID - Cortex-M Interrupt Data

The following is the desired end goal of this project. We're not there yet.

## Stage 1 - Moving data to interrupts

```rust
use nrf52832_pac::Interrupt;
use something::Queue;

// Done at global scope, only specify types
cmim!(
    Interrupt::RADIO: bool,
    Interrupt::TIMER0: u128,
    Interrupt::UARTE0_UARTE: Queue::Producer,
);

fn main() {
    let (prod, cons) = Queue::new().split();

    // Sets the value described in `cmim!()`.
    // This is a "move" operation.
    // If the interrupt is currently active, an Error is returned.
    cmim_set!(
        Interrupt::UARTE0_UARTE,
        prod
    ).unwrap();

    NVIC::enable(Interrupt::UARTE0_UARTE);

    loop {
        let _ = cons.pop();
        // ..
    }
}

#[interrupt]
fn UARTE0_UARTE() {
    // Gets a mutable reference to the value described in `cmim!()`
    // This is a "borrow" operation.
    // This checks the currently active interrupt. If Interrupt::UARTE0_UARTE is not active, an error is returned
    // There is no other mutex.
    let data: &mut Producer = cmim_get!(Interrupt::UARTE0_UARTE).unwrap();
    data.push(0x00);
}
```

## Stage 2 - Sharing data with a single interrupt

```rust
use nrf52832_pac::Interrupt;

// Done at global scope, only specify types
cmim!(
    Interrupt::RADIO: bool,
    Interrupt::TIMER0: u128,
    Interrupt::UARTE0_UARTE: bbqueue::Producer,
);

fn main() {
    // Same as above for setting, Interrupt must be disabled
    cmim_set!(
        Interrupt::RADIO,
        false
    ).unwrap();

    NVIC::enable(Interrupt::RADIO);

    loop {
        // Access the data in a critical section. Radio is disabled during the closure
        // This can only be called from non-interrupt context. If ANY interrupt is active,
        // an error is returned. This prevents higher prio interrupts messing with the data
        let data_copy = cmim_borrow!(
            Interrupt::RADIO,
            |data: &mut bool| {
                // trigger some flag
                *data = true;
            }
        ).unwrap();
    }
}

#[interrupt]
fn RADIO() {
    // Gets a mutable reference to the value described in `cmim!()`
    // This is a "borrow" operation.
    // This checks the currently active interrupt. If Interrupt::RADIO is not active, an error is returned
    // There is no other mutex.
    let data: &mut bool = cmim_get!(Interrupt::RADIO).unwrap();

    if *data {
        // ...
    }
}
```

## Stage 3 - Sharing data between interrupts

I have no idea how to do this without the possibility of deadlock. Maybe specify multiple interrupts in `cmim!()`, and critical section all of them at once? Maybe do priority elevation like RTFM?


# Alternative Future:

A single macro to create an interrupt safe singleton.

```rust
static mut $NAME: Option<$dat> = None;
struct $VARIABLE_NAME {
    const INTERRUPT: Interrupt = $int;

    fn set_data(data: $dat) -> Result<(), Error> {

    }

    fn take_app() -> Result<$token_ty, Error> {
        // make sure interrupt isn't active
        // basically just a token for a ZST
    }

    fn borrow_interrupt(FnOnce) -> Result<()>
    {
        // makes sure interrupt is active and enabled
    }
}

struct $token_ty {
    const INTERRUPT: Interrupt = $int;
    fn borrow_mut(&mut self, FnOnce) -> Result<()> {

    }
}

```

```rust
pub mod $NAME {
    pub struct AppToken {
        _private: ()
    }

    static mut $NAME: Option<$dat_ty> = None;

    mod flag {
        static $NAME: AtomicBool = AtomicBool::new(false);
    }

    impl AppToken {
        pub fn set_initial(data: $dat_ty) -> Result<AppToken, $dat_ty> {
            if int_is_enabled() || super::flag::$NAME.load(::core::sync::atomic::Ordering::SeqCst) {
                return Err(data);
            }

            if $NAME.is_none() {
                unsafe {
                    $NAME = Some(data);
                }
                AppToken { _private: () }
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
            let mut enabled = int_is_enabled();
            if enabled {
                disable_int();
            }
            if int_is_active() || $NAME.is_none() {
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

        fn modify_int_context<F>(f: F) -> Result<(), ()>
        where
            for<'w> F: FnOnce(&'w mut $dat) -> &'w mut $dat,
        {
            if !int_is_active() || $name.is_none() || super::flag::$NAME.swap(true, ::core::sync::atomic::Ordering::SeqCst) {
                return Err(());
            }

            unsafe {
                f($name.as_mut().unwrap());
            }

            assert(super::flag::$NAME.swap(false, ::core::sync::atomic::Ordering::SeqCst));

        }
    }

    pub struct Data {
        _private: ()
    }

    impl Data {}

    fn int_is_enabled() -> bool
    {

        let nr = $int.nr();
        let mask = 1 << (nr % 32);

        // NOTE(unsafe) atomic read with no side effects
        unsafe { ((*::cortex_m::peripheral::NVIC::ptr()).iser[usize::from(nr / 32)].read() & mask) == mask }
    }

    fn int_is_active() -> bool
    {
        let nr = $int.nr();
        let mask = 1 << (nr % 32);

        // NOTE(unsafe) atomic read with no side effects
        unsafe { ((*::cortex_m::peripheral::NVIC::ptr()).iabr[usize::from(nr / 32)].read() & mask) == mask }
    }

    fn disable_int()
    {
        let nr = interrupt.nr();

        unsafe { (*::cortex_m::peripheral::NVIC::ptr()).icer[usize::from(nr / 32)].write(1 << (nr % 32)) }
    }

    /// Enables `interrupt`
    fn enable_int()
    {
        let nr = interrupt.nr();

        unsafe { (*::cortex_m::peripheral::NVIC::ptr()).iser[usize::from(nr / 32)].write(1 << (nr % 32)) }
    }
}
```
