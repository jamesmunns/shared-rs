# `Shared`

**A macro for safely sharing data between application and interrupt context on cortex-m systems**

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
