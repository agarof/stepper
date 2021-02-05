mod set_direction;
mod set_step_mode;
mod step;

pub use self::{
    set_direction::SetDirectionFuture, set_step_mode::SetStepModeFuture,
    step::StepFuture,
};

use core::convert::TryFrom;

use embedded_hal::timer;
use embedded_time::duration::Nanoseconds;

use crate::{
    traits::{
        EnableDirectionControl, EnableStepControl, EnableStepModeControl,
        SetDirection, SetStepMode, Step,
    },
    Direction,
};

/// Unified stepper motor interface
///
/// Wraps a driver that interfaces with the motor-controlling hardware and
/// abstracts over it, providing an interface that works the same, no matter
/// what kind of hardware controls the stepper motor.
///
/// You can construct an instance of this type using [`Stepper::from_driver`].
///
/// # Nomenclature
///
/// This structs wraps a software component that interfaces with hardware that
/// controls a stepper motor. That software component is called a "driver",
/// because it "drives" the hardware it interfaces with.
///
/// The driven hardware typically comes in two forms:
///
/// - A low-level chip controlled by STEP and DIR signals, often called a
//    stepper driver (yes, somewhat confusing) or stepper controller.
/// - A higher-level chip, typically controlled through some serial interface,
///   often called a motion controller.
///
/// In practice, a given product can cleanly fall into one of the two camps,
/// both, or anything in between.
///
/// # Notes on timer use
///
/// Some of this struct's methods take a timer argument. This is expected to be
/// an implementation of [`embedded_hal::timer::CountDown`], with the additional
/// requirement that `CountDown::Time` has a `TryFrom<Nanoseconds>`
/// implementation, where `Nanoseconds` refers to
/// [`embedded_time::duration::Nanoseconds`].
///
/// Not every `CountDown` implementation provides this for its `Time` type, so
/// it might be necessary that the user either adds this `embedded_time`
/// integration to the HAL library they are using, or provides a wrapper around
/// the `CountDown` implementation in their own code, adding the conversion
/// there.
///
/// Every method that takes a timer argument internally performs the conversion
/// from `Nanoseconds` to the timers `Time` type. Since the nanosecond values
/// are constant and the `CountDown` implementation is known statically, the
/// compiler should have enough information to perform this conversion at
/// compile-time.
///
/// Unfortunately there is currently no way to make sure that this optimization
/// actually happens. Additions like [RFC 2632], [RFC 2920], and possibly others
/// along those lines, could help with this in the future. For now, users must
/// manually inspect the generated code and tweak optimization settings (and
/// possibly the HAL-specific conversion code), if this level of performance is
/// required.
///
/// [RFC 2632]: https://github.com/rust-lang/rfcs/pull/2632
/// [RFC 2920]: https://github.com/rust-lang/rfcs/pull/2920
pub struct Stepper<Driver> {
    driver: Driver,
}

impl<Driver> Stepper<Driver> {
    /// Create a new `Stepper` instance from a driver
    pub fn from_driver(driver: Driver) -> Self {
        Self { driver }
    }

    /// Access a reference to the wrapped driver
    ///
    /// Can be used to access driver-specific functionality that can't be
    /// provided by `Stepper`'s abstract interface.
    pub fn inner(&self) -> &Driver {
        &self.driver
    }

    /// Access a mutable reference to the wrapped driver or controller
    ///
    /// Can be used to access driver-specific functionality that can't be
    /// provided by `Stepper`'s abstract interface.
    pub fn inner_mut(&mut self) -> &mut Driver {
        &mut self.driver
    }

    /// Release the wrapped driver
    ///
    /// Drops this instance of `Stepper` and returns the wrapped driver.
    pub fn release(self) -> Driver {
        self.driver
    }

    /// Enable microstepping mode control
    ///
    /// Consumes this instance of `Stepper` and returns a new instance that
    /// provides control over the microstepping mode. Once this method has been
    /// called, the [`Stepper::set_step_mode`] method becomes available.
    ///
    /// Takes the hardware resources that are required for controlling the
    /// microstepping mode as an argument. What exactly those are depends on the
    /// specific driver. Typically they are the output pins that are connected
    /// to the mode pins of the driver.
    ///
    /// This method is only available, if the driver supports enabling step mode
    /// control. It might no longer be available, once step mode control has
    /// been enabled.
    pub fn enable_step_mode_control<Resources, Timer>(
        self,
        res: Resources,
        initial: <Driver::WithStepModeControl as SetStepMode>::StepMode,
        timer: &mut Timer,
    ) -> Result<
        Stepper<Driver::WithStepModeControl>,
        Error<
            <Driver::WithStepModeControl as SetStepMode>::Error,
            <Timer::Time as TryFrom<Nanoseconds>>::Error,
            Timer::Error,
        >,
    >
    where
        Driver: EnableStepModeControl<Resources>,
        Timer: timer::CountDown,
        Timer::Time: TryFrom<Nanoseconds>,
    {
        let mut self_ = Stepper {
            driver: self.driver.enable_step_mode_control(res),
        };
        self_.set_step_mode(initial, timer).wait()?;

        Ok(self_)
    }

    /// Sets the microstepping mode
    ///
    /// This method is only available, if the wrapped driver supports
    /// microstepping, and supports setting the step mode through software. Some
    /// hardware might not support microstepping at all, or only allow setting
    /// the step mode by changing physical switches.
    ///
    /// You might need to call [`Stepper::enable_step_mode_control`] to make
    /// this method available.
    pub fn set_step_mode<'r, Timer>(
        &'r mut self,
        step_mode: Driver::StepMode,
        timer: &'r mut Timer,
    ) -> SetStepModeFuture<'r, Driver, Timer>
    where
        Driver: SetStepMode,
        Timer: timer::CountDown,
        Timer::Time: TryFrom<Nanoseconds>,
    {
        SetStepModeFuture::new(step_mode, self, timer)
    }

    /// Enable direction control
    ///
    /// Consumes this instance of `Stepper` and returns a new instance that
    /// provides control over the motor direction. Once this method has been
    /// called, the [`Stepper::set_direction`] method becomes available.
    ///
    /// Takes the hardware resources that are required for controlling the
    /// direction as an argument. What exactly those are depends on the specific
    /// driver. Typically it's going to be the output pin that is connected to
    /// the hardware's DIR pin.
    ///
    /// This method is only available, if the driver supports enabling direction
    /// control. It might no longer be available, once direction control has
    /// been enabled.
    pub fn enable_direction_control<Resources, Timer>(
        self,
        res: Resources,
        initial: Direction,
        timer: &mut Timer,
    ) -> Result<
        Stepper<Driver::WithDirectionControl>,
        Error<
            <Driver::WithDirectionControl as SetDirection>::Error,
            <Timer::Time as TryFrom<Nanoseconds>>::Error,
            Timer::Error,
        >,
    >
    where
        Driver: EnableDirectionControl<Resources>,
        Timer: timer::CountDown,
        Timer::Time: TryFrom<Nanoseconds>,
    {
        let mut self_ = Stepper {
            driver: self.driver.enable_direction_control(res),
        };
        self_.set_direction(initial, timer).wait()?;

        Ok(self_)
    }

    /// Set direction for future movements
    ///
    /// You might need to call [`Stepper::enable_direction_control`] to make
    /// this method available.
    pub fn set_direction<'r, Timer>(
        &'r mut self,
        direction: Direction,
        timer: &'r mut Timer,
    ) -> SetDirectionFuture<'r, Driver, Timer>
    where
        Driver: SetDirection,
        Timer: timer::CountDown,
        Timer::Time: TryFrom<Nanoseconds>,
    {
        SetDirectionFuture::new(direction, self, timer)
    }

    /// Enable step control
    ///
    /// Consumes this instance of `Stepper` and returns a new instance that
    /// provides control over stepping the motor. Once this method has been
    /// called, the [`Stepper::step`] method becomes available.
    ///
    /// Takes the hardware resources that are required for controlling the
    /// direction as an argument. What exactly those are depends on the specific
    /// driver. Typically it's going to be the output pin that is connected to
    /// the hardware's STEP pin.
    ///
    /// This method is only available, if the driver/controller supports
    /// enabling step control. It might no longer be available, once step
    /// control has been enabled.
    pub fn enable_step_control<Resources>(
        self,
        res: Resources,
    ) -> Stepper<Driver::WithStepControl>
    where
        Driver: EnableStepControl<Resources>,
    {
        Stepper {
            driver: self.driver.enable_step_control(res),
        }
    }

    /// Rotates the motor one (micro-)step in the given direction
    ///
    /// Steps the motor one step in the direction that was previously set,
    /// according to current microstepping configuration. To achieve a specific
    /// speed, the user must call this method at an appropriate frequency.
    ///
    /// You might need to call [`Stepper::enable_step_control`] to make this
    /// method available.
    pub fn step<'r, Timer>(
        &'r mut self,
        timer: &'r mut Timer,
    ) -> StepFuture<'r, Driver, Timer>
    where
        Driver: Step,
        Timer: timer::CountDown,
        Timer::Time: TryFrom<Nanoseconds>,
    {
        StepFuture::new(self, timer)
    }

    /// Returns the step pulse length of the wrapped driver/controller
    ///
    /// The pulse length is also available through the [`Step`] trait. This
    /// method provides a more convenient way to access it.
    ///
    /// You might need to call [`Stepper::enable_step_control`] to make this
    /// method available.
    pub fn pulse_length(&self) -> Nanoseconds
    where
        Driver: Step,
    {
        Driver::PULSE_LENGTH
    }
}

/// An error that can occur while using this API
#[derive(Debug, Eq, PartialEq)]
pub enum Error<PinError, TimeConversionError, TimerError> {
    /// An error originated from using the [`OutputPin`] trait
    ///
    /// [`OutputPin`]: embedded_hal::digital::OutputPin
    Pin(PinError),

    /// An error occurred while converting time to timer ticks
    TimeConversion(TimeConversionError),

    /// An error originated from working with a timer
    Timer(TimerError),
}
