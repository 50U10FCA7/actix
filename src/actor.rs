use futures::{Future, Stream};

use fut::ActorFuture;
use message::Response;
use address::ActorAddress;
use context::{ActorFutureCell, ActorStreamCell};


#[allow(unused_variables)]
/// Actors are objects which encapsulate state and behavior.
///
/// Actors run within specific execution context
/// [Context<A>](https://fafhrd91.github.io/actix/actix/struct.Context.html).
/// Context object is available only during execution. Each actor has separate
/// execution context. Also execution context controls lifecycle of an actor.
///
/// Actors communicate exclusively by exchanging messages. Sender actor can
/// wait for response. Actors are not referenced dirrectly, but by
/// non thread safe [Address<A>](https://fafhrd91.github.io/actix/actix/struct.Address.html)
/// or thread safe address
/// [`SyncAddress<A>`](https://fafhrd91.github.io/actix/actix/struct.SyncAddress.html)
/// To be able to handle specific message actor has to provide
/// [`Handler<M>`](
/// file:///Users/nikki/personal/ctx/target/doc/actix/trait.Handler.html)
/// implementation for this message. All messages are statically typed. Message could be
/// handled in asynchronous fasion. Actor can spawn other actors or add futures or
/// streams to execution context. Actor trait provides several method that allows
/// to control actor lifecycle.
///
/// # Actor lifecycle
///
/// ## Started
///
/// Actor starts in `Started` state, during this state `started` method get called.
///
/// ## Running
///
/// Aftre Actor's method `started` get called, actor transitiones to `Running` state.
/// Actor can stay in `running` state indefinitely long.
///
/// ## Stopping
///
/// Actor execution state changes to `stopping` state in following situations,
///
/// * `Context::stop` get called by actor itself
/// * all addresses to the actor get dropped
/// * no evented objects are registered in context.
///
/// Actor could restore from `stopping` state to `running` state by creating new
/// address or adding evented object, like future or stream, in `Actor::stopping` method.
///
/// ## Stopped
///
/// If actor does not modify execution context during stooping state actor state changes
/// to `Stopped`. This state is considered final and at this point actor get dropped.
///
pub trait Actor: Sized + 'static {

    /// Actor execution context type
    type Context: BaseContext<Self>;

    /// Method is called when actor get polled first time.
    fn started(&mut self, ctx: &mut Self::Context) {}

    /// Method is called after an actor is in STOPPING state. There could be several
    /// reasons for stopping. Context::stop get called by actor itself.
    /// All addresses to current actor get dropped and no more evented objects
    /// left in context. Actor could restore from stopping state to running state
    /// by creating new address or adding future or stream to current content.
    fn stopping(&mut self, ctx: &mut Self::Context) {}

    /// Method is called after an actor is stopped, it can be used to perform
    /// any needed cleanup work or spawning more actors.
    fn stopped(&mut self, ctx: &mut Self::Context) {}
}

#[allow(unused_variables)]
/// Actors with ability to restart after failure
///
/// Supervised actors can be managed by
/// [Supervisor](https://fafhrd91.github.io/actix/actix/struct.Supervisor.html)
/// Livecycle events are extended with `restarting` state for supervised actors.
/// If actor failes supervisor create new execution context and restart actor.
/// `restarting` method is called during restart. After call to this method
/// Actor execute state changes to `Started` and normal lifecycle process starts.
pub trait Supervised: Actor {

    /// Method called when supervisor restarting failed actor
    fn restarting(&mut self, ctx: &mut <Self as Actor>::Context) {}
}

/// Message handler
///
/// `Handler` implementation is a general way how to handle
/// incoming messages, streams, futures.
///
/// `M` is message which can be handled by actor
/// `E` optional error type, if message handler is used for handling messages
///  from Future or Stream, then `E` type has to be set to correspondent `Error` type.
#[allow(unused_variables)]
pub trait Handler<M, E=()> where Self: Actor + ResponseType<M>
{
    /// Method is called for every message received by this Actor
    fn handle(&mut self, msg: M, ctx: &mut Self::Context) -> Response<Self, M>;

    /// Method is called on error. By default it does nothing.
    fn error(&mut self, err: E, ctx: &mut Self::Context) {}
}

/// Response type
pub trait ResponseType<M> where Self: Actor {

    /// The type of value that this message will resolved with if it is successful.
    type Item;

    /// The type of error that this message will resolve with if it fails in a normal fashion.
    type Error;
}

/// Stream handler
///
/// `StreamHandler` is an extension of a `Handler` with several stream specific
/// methods.
#[allow(unused_variables)]
pub trait StreamHandler<M, E>: Handler<M, E> + ResponseType<M>
    where Self: Actor
{
    /// Method is called when stream get polled first time.
    fn started(&mut self, ctx: &mut Self::Context) {}

    /// Method is called when stream finishes.
    fn finished(&mut self, ctx: &mut Self::Context) {}
}

/// Actor execution state
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ActorState {
    /// Actor is started.
    Started,
    /// Actor is running.
    Running,
    /// Actor is stopping.
    Stopping,
    /// Actor is stopped.
    Stopped,
}

pub trait BaseContext<A>: Sized where A: Actor<Context=Self> {

    /// Actor execution state
    fn state(&self) -> ActorState;

    /// Get actor address
    fn address<Address>(&mut self) -> Address where A: ActorAddress<A, Address>
    {
        <A as ActorAddress<A, Address>>::get(self)
    }
}

pub trait AsyncContext<A>: BaseContext<A> where A: Actor<Context=Self>
{
    /// Spawn async future into context
    fn spawn<F>(&mut self, fut: F)
        where F: ActorFuture<Item=(), Error=(), Actor=A> + 'static;

    /// This method allow to handle Future in similar way as normal actor message.
    ///
    /// ```rust
    /// extern crate actix;
    /// extern crate futures;
    /// extern crate tokio_core;
    ///
    /// use std::time::Duration;
    /// use futures::Future;
    /// use tokio_core::reactor::Timeout;
    /// use actix::prelude::*;
    ///
    /// // Message
    /// struct Ping;
    ///
    /// struct MyActor;
    ///
    /// impl ResponseType<Ping> for MyActor {
    ///     type Item = ();
    ///     type Error = ();
    /// }
    ///
    /// impl Handler<Ping, std::io::Error> for MyActor {
    ///     fn error(&mut self, err: std::io::Error, ctx: &mut Context<MyActor>) {
    ///         println!("Error: {}", err);
    ///     }
    ///     fn handle(&mut self, msg: Ping, ctx: &mut Context<MyActor>) -> Response<Self, Ping> {
    ///         println!("PING");
    ///         Response::Reply(())
    ///     }
    /// }
    ///
    /// impl Actor for MyActor {
    ///    type Context = Context<Self>;
    ///
    ///    fn started(&mut self, ctx: &mut Context<Self>) {
    ///        ctx.add_future(
    ///            Timeout::new(Duration::new(0, 1000), Arbiter::handle()).unwrap()
    ///                .map(|_| Ping)
    ///        );
    ///    }
    /// }
    /// fn main() {}
    /// ```
    fn add_future<F>(&mut self, fut: F)
        where F: Future + 'static, A: Handler<F::Item, F::Error>
    {
        if self.state() == ActorState::Stopped {
            error!("Context::add_future called for stopped actor.");
        } else {
            self.spawn(ActorFutureCell::new(fut))
        }
    }

    /// This method is similar to `add_future` but works with streams.
    fn add_stream<S>(&mut self, fut: S)
        where S: Stream + 'static,
              A: Handler<S::Item, S::Error> + StreamHandler<S::Item, S::Error>
    {
        if self.state() == ActorState::Stopped {
            error!("Context::add_stream called for stopped actor.");
        } else {
            self.spawn(ActorStreamCell::new(fut))
        }
    }
}
