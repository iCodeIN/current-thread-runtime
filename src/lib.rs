extern crate tokio_timer;

use tokio::executor::current_thread::CurrentThread;
pub use tokio_executor::park::{Park, Unpark};

pub use tokio::reactor::Reactor;
pub use tokio_timer::clock::Clock;
pub use tokio_timer::timer::{DefaultTimerImpl, Timer, TimerImpl};

use std::io;

mod runtime;

pub use crate::runtime::{Runtime, RunError, Handle};

/// Builds a Single-threaded runtime with custom configuration values.
///
/// Methods can be chained in order to set the configuration values. The
/// Runtime is constructed by calling [`build`].
///
/// New instances of `Builder` are obtained via [`Builder::new`].
///
/// See function level documentation for details on the various configuration
/// settings.
///
/// [`build`]: #method.build
/// [`Builder::new`]: #method.new
///
/// # Examples
///
/// ```
/// extern crate tokio;
/// extern crate tokio_timer;
///
/// use tokio::runtime::current_thread::Builder;
/// use tokio_timer::clock::Clock;
///
/// # pub fn main() {
/// // build Runtime
/// let runtime = Builder::new()
///     .clock(Clock::new())
///     .build();
/// // ... call runtime.run(...)
/// # let _ = runtime;
/// # }
/// ```
pub struct Builder<T = DefaultTimerImpl<Reactor>> {
    /// The clock to use
    clock: Clock,

    new_timer: Box<Fn(Reactor, Clock) -> Timer<Reactor, Clock, T>>,
}

impl Builder {
    /// Returns a new runtime builder initialized with default configuration
    /// values.
    ///
    /// Configuration methods can be chained on the return value.
    pub fn new() -> Builder {
        Builder {
            clock: Clock::new(),
            new_timer: Box::new(|reactor, clock| Timer::new_with_now(reactor, clock)),
        }
    }
}

impl<T> Builder<T>
where
    T: TimerImpl<Reactor>,
{
    pub fn with_timer<F>(new_timer: F) -> Self
        where F: Fn(Reactor, Clock) -> Timer<Reactor, Clock, T> + 'static
    {
        Builder {
            clock: Clock::new(),
            new_timer: Box::new(new_timer),
        }
    }

    /// Set the `Clock` instance that will be used by the runtime.
    pub fn clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = clock;
        self
    }

    /// Create the configured `Runtime`.
    pub fn build(&mut self) -> io::Result<Runtime<Timer<Reactor, Clock, T>>> {
        self.build_with_park(|park| park).map(|(rt, _)| rt)
    }

    /// Create the configured `Runtime`.
    pub fn build_with_park<U: Park, F: FnOnce(Timer<Reactor, Clock, T>) -> U>(
        &mut self,
        new_park: F,
    ) -> io::Result<(Runtime<U>, U::Unpark)> {
        // We need a reactor to receive events about IO objects from kernel
        let reactor = Reactor::new()?;
        let reactor_handle = reactor.handle();

        // Place a timer wheel on top of the reactor. If there are no timeouts to fire, it'll let the
        // reactor pick up some new external events.
        let timer = (self.new_timer)(reactor, self.clock.clone());
        let timer_handle = timer.handle();

        let park = new_park(timer);
        let unpark = park.unpark();

        // And now put a single-threaded executor on top of the timer. When there are no futures ready
        // to do something, it'll let the timer or the reactor to generate some new stimuli for the
        // futures to continue in their life.
        let executor = CurrentThread::new_with_park(park);

        let runtime = Runtime::new2(reactor_handle, timer_handle, self.clock.clone(), executor);

        Ok((runtime, unpark))
    }
}
