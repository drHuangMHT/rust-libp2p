use std::{collections::VecDeque, task::Poll};

use libp2p_core::{Multiaddr, PeerId};
use libp2p_swarm::{dummy, NetworkBehaviour};

use crate::store::Store;

/// Events generated by [`Behaviour`] and emitted back to [`Swarm`](libp2p_swarm::Swarm).
#[derive(Debug, Clone)]
pub enum Event<T> {
    /// The peer's record has been updated.  
    /// Manually updating a record will always emit this event
    /// even if it provides no new information.
    RecordUpdated {
        /// The peer that has an update.
        peer: PeerId,
    },
    /// Event from the internal store.
    Store(T),
}

/// Behaviour that maintains a peer address book.
///
/// Usage:
/// ```
/// use libp2p::swarm::NetworkBehaviour;
/// use libp2p_peer_store::{memory_store::MemoryStore, Behaviour};
///
/// // `identify::Behaviour` broadcasts listen addresses of the peer,
/// // `peer_store::Behaviour` will then capture the resulting
/// // `FromSwarm::NewExternalAddrOfPeer` and add the addresses
/// // to address book.
/// #[derive(NetworkBehaviour)]
/// struct ComposedBehaviour {
///     peer_store: Behaviour<MemoryStore>,
///     identify: libp2p::identify::Behaviour,
/// }
/// ```
pub struct Behaviour<S: Store> {
    /// The internal store.
    store: S,
    /// Pending Events to be emitted back to [`Swarm`](libp2p_swarm::Swarm).
    pending_events: VecDeque<Event<S::FromStore>>,
}

impl<'a, S> Behaviour<S>
where
    S: Store + 'static,
{
    /// Build a new [`Behaviour`] with the given store.
    pub fn new(store: S) -> Self {
        Self {
            store,
            pending_events: VecDeque::new(),
        }
    }

    /// Try to get all observed address of the given peer.  
    /// Returns `None` when the peer is not in the store.
    pub fn address_of_peer<'b>(
        &'a self,
        peer: &'b PeerId,
    ) -> Option<impl Iterator<Item = &'a Multiaddr> + use<'a, 'b, S>> {
        self.store.addresses_of_peer(peer)
    }

    /// Get an immutable reference to the internal store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a mutable reference to the internal store.
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    fn handle_store_event(&mut self, event: crate::store::Event<<S as Store>::FromStore>) {
        use crate::store::Event::*;
        match event {
            RecordUpdated(peer) => self.pending_events.push_back(Event::RecordUpdated { peer }),
            Store(ev) => self.pending_events.push_back(Event::Store(ev)),
        }
    }
}

impl<S> NetworkBehaviour for Behaviour<S>
where
    S: Store + 'static,
    <S as Store>::FromStore: Send + Sync,
{
    type ConnectionHandler = dummy::ConnectionHandler;

    type ToSwarm = Event<S::FromStore>;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p_swarm::ConnectionId,
        _peer: libp2p_core::PeerId,
        _local_addr: &libp2p_core::Multiaddr,
        _remote_addr: &libp2p_core::Multiaddr,
    ) -> Result<libp2p_swarm::THandler<Self>, libp2p_swarm::ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _connection_id: libp2p_swarm::ConnectionId,
        maybe_peer: Option<PeerId>,
        _addresses: &[Multiaddr],
        _effective_role: libp2p_core::Endpoint,
    ) -> Result<Vec<Multiaddr>, libp2p_swarm::ConnectionDenied> {
        if maybe_peer.is_none() {
            return Ok(Vec::new());
        }
        let peer = maybe_peer.expect("already handled");
        Ok(self
            .store
            .addresses_of_peer(&peer)
            .map(|i| i.cloned().collect())
            .unwrap_or_default())
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p_swarm::ConnectionId,
        _peer: libp2p_core::PeerId,
        _addr: &libp2p_core::Multiaddr,
        _role_override: libp2p_core::Endpoint,
        _port_use: libp2p_core::transport::PortUse,
    ) -> Result<libp2p_swarm::THandler<Self>, libp2p_swarm::ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: libp2p_swarm::FromSwarm) {
        self.store.on_swarm_event(&event);
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: libp2p_core::PeerId,
        _connection_id: libp2p_swarm::ConnectionId,
        _event: libp2p_swarm::THandlerOutEvent<Self>,
    ) {
        unreachable!("No event will be produced by a dummy handler.")
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p_swarm::ToSwarm<Self::ToSwarm, libp2p_swarm::THandlerInEvent<Self>>>
    {
        if let Some(ev) = self.store.poll(cx) {
            self.handle_store_event(ev);
        };

        if let Some(ev) = self.pending_events.pop_front() {
            return Poll::Ready(libp2p_swarm::ToSwarm::GenerateEvent(ev));
        }
        Poll::Pending
    }
}
