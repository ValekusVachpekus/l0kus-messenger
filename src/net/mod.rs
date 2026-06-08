//! Сетевой слой: libp2p Swarm, поведение и wire-протокол.

pub mod behaviour;
pub mod protocol;
pub mod swarm;

pub use behaviour::{Behaviour, BehaviourEvent};
