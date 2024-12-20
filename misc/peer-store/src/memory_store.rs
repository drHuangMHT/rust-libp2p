use std::{
    collections::HashMap,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use libp2p_core::{Multiaddr, PeerId};
use libp2p_swarm::FromSwarm;

use super::{store::Event, Store};
use crate::{store::AddressSource, Behaviour};

/// A in-memory store.
#[derive(Default)]
pub struct MemoryStore {
    /// An address book of peers regardless of their status(connected or not).
    address_book: HashMap<PeerId, record::PeerAddressRecord>,
    config: Config,
}

impl MemoryStore {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
}

impl<'a> Store<'a> for MemoryStore {
    type AddressRecord = AddressRecord<'a>;

    fn update_address(
        &mut self,
        peer: &PeerId,
        address: &Multiaddr,
        source: AddressSource,
        should_expire: bool,
    ) -> bool {
        if let Some(record) = self.address_book.get_mut(peer) {
            return record.update_address(address, source, should_expire);
        }
        let mut new_record = record::PeerAddressRecord::new(self.config.record_capacity);
        new_record.update_address(address, source, should_expire);
        self.address_book.insert(*peer, new_record);
        true
    }

    fn remove_address(&mut self, peer: &PeerId, address: &Multiaddr) -> bool {
        if let Some(record) = self.address_book.get_mut(peer) {
            return record.remove_address(address);
        }
        false
    }

    fn on_swarm_event(&mut self, swarm_event: &FromSwarm) -> Option<Event> {
        match swarm_event {
            FromSwarm::NewExternalAddrOfPeer(info) => {
                if self.update_address(&info.peer_id, info.addr, AddressSource::Behaviour, true) {
                    return Some(Event::RecordUpdated(info.peer_id));
                }
                None
            }
            FromSwarm::ConnectionEstablished(info) => {
                let mut is_record_updated = false;
                for failed_addr in info.failed_addresses {
                    is_record_updated |= self.remove_address(&info.peer_id, failed_addr);
                }
                is_record_updated |= self.update_address(
                    &info.peer_id,
                    info.endpoint.get_remote_address(),
                    AddressSource::DirectConnection,
                    false,
                );
                if is_record_updated {
                    return Some(Event::RecordUpdated(info.peer_id));
                }
                None
            }
            _ => None,
        }
    }

    fn addresses_of_peer(&self, peer: &PeerId) -> Option<impl Iterator<Item = &Multiaddr>> {
        self.address_book
            .get(peer)
            .map(|record| record.records().map(|r| r.address))
    }

    fn check_ttl(&mut self) {
        let now = Instant::now();
        for r in &mut self.address_book.values_mut() {
            r.check_ttl(now, self.config.record_ttl);
        }
    }
}

impl Behaviour<MemoryStore> {
    /// Get all stored address records of the peer.
    pub fn address_record_of_peer(
        &self,
        peer: &PeerId,
    ) -> Option<impl Iterator<Item = super::AddressRecord>> {
        self.store()
            .address_book
            .get(peer)
            .map(|record| record.records())
    }
}

pub struct Config {
    /// TTL for a record.
    record_ttl: Duration,
    /// The capacaity of a record store.  
    /// The least used record will be discarded when the store is full.
    record_capacity: NonZeroUsize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            record_ttl: Duration::from_secs(600),
            record_capacity: NonZeroUsize::try_from(8).expect("8 > 0"),
        }
    }
}

pub struct AddressRecord<'a> {
    /// The last time we saw this address.
    last_seen: &'a Instant,
    /// The address of this record.
    pub address: &'a Multiaddr,
    /// How we observed the address.
    pub source: AddressSource,
    /// Whether the address expires.
    pub should_expire: bool,
}
impl AddressRecord<'_> {
    /// How much time has passed since the address is last reported wrt. the given instant.  
    pub fn last_seen_since(&self, now: Instant) -> Duration {
        now.duration_since(*self.last_seen)
    }
    /// How much time has passed since the address is last reported wrt. current time.
    pub fn last_seen(&self) -> Duration {
        let now = Instant::now();
        now.duration_since(*self.last_seen)
    }
}

mod record {
    use lru::LruCache;

    use super::*;

    pub(crate) struct PeerAddressRecord {
        /// A LRU(Least Recently Used) cache for addresses.  
        /// Will delete the least-recently-used record when full.
        addresses: LruCache<Multiaddr, AddressRecord>,
    }
    impl PeerAddressRecord {
        pub(crate) fn new(capacity: NonZeroUsize) -> Self {
            Self {
                addresses: LruCache::new(capacity),
            }
        }
        pub(crate) fn records(&self) -> impl Iterator<Item = super::AddressRecord> {
            self.addresses
                .iter()
                .map(|(address, record)| super::AddressRecord {
                    address,
                    last_seen: &record.last_seen,
                    source: record.source,
                    should_expire: record.should_expire,
                })
        }
        pub(crate) fn update_address(
            &mut self,
            address: &Multiaddr,
            source: AddressSource,
            should_expire: bool,
        ) -> bool {
            if let Some(record) = self.addresses.get_mut(address) {
                record.update_last_seen();
                return false;
            }
            // new record won't call `Instant::now()` twice
            self.addresses.get_or_insert(address.clone(), || {
                AddressRecord::new(source, should_expire)
            });
            true
        }
        pub(crate) fn remove_address(&mut self, address: &Multiaddr) -> bool {
            self.addresses.pop(address).is_some()
        }
        pub(crate) fn check_ttl(&mut self, now: Instant, ttl: Duration) {
            let mut records_to_be_deleted = Vec::new();
            for (k, record) in self.addresses.iter() {
                if record.is_expired(now, ttl) {
                    records_to_be_deleted.push(k.clone());
                }
            }
            for k in records_to_be_deleted {
                self.addresses.pop(&k);
            }
        }
    }

    pub(crate) struct AddressRecord {
        /// The time when the address is last seen.
        last_seen: Instant,
        /// How the address is discovered.
        source: AddressSource,
        /// Whether the address will expire.
        should_expire: bool,
    }
    impl AddressRecord {
        pub(crate) fn new(source: AddressSource, should_expire: bool) -> Self {
            Self {
                last_seen: Instant::now(),
                source,
                should_expire,
            }
        }
        pub(crate) fn update_last_seen(&mut self) {
            self.last_seen = Instant::now();
        }
        pub(crate) fn is_expired(&self, now: Instant, ttl: Duration) -> bool {
            self.should_expire && now.duration_since(self.last_seen) > ttl
        }
    }
}

#[cfg(test)]
mod test {
    use std::{num::NonZeroUsize, str::FromStr, thread, time::Duration};

    use libp2p_core::{Multiaddr, PeerId};

    use super::{Config, MemoryStore};
    use crate::Store;

    #[test]
    fn record_expire() {
        let config = Config {
            record_capacity: NonZeroUsize::try_from(4).expect("4 > 0"),
            record_ttl: Duration::from_millis(1),
        };
        let mut store = MemoryStore::new(config);
        let fake_peer = PeerId::random();
        let addr_no_expire = Multiaddr::from_str("/ip4/127.0.0.1").expect("parsing to succeed");
        let addr_should_expire = Multiaddr::from_str("/ip4/127.0.0.2").expect("parsing to succeed");
        store.update_address(
            &fake_peer,
            &addr_no_expire,
            crate::store::AddressSource::Manual,
            false,
        );
        store.update_address(
            &fake_peer,
            &addr_should_expire,
            crate::store::AddressSource::Manual,
            true,
        );
        thread::sleep(Duration::from_millis(2));
        store.check_ttl();
        assert!(!store
            .addresses_of_peer(&fake_peer)
            .expect("peer to be in the store")
            .any(|r| *r == addr_should_expire));
        assert!(store
            .addresses_of_peer(&fake_peer)
            .expect("peer to be in the store")
            .any(|r| *r == addr_no_expire));
    }

    #[test]
    fn recent_use_bubble_up() {
        let mut store = MemoryStore::new(Default::default());
        let fake_peer = PeerId::random();
        let addr1 = Multiaddr::from_str("/ip4/127.0.0.1").expect("parsing to succeed");
        let addr2 = Multiaddr::from_str("/ip4/127.0.0.2").expect("parsing to succeed");
        store.update_address(
            &fake_peer,
            &addr1,
            crate::store::AddressSource::Manual,
            false,
        );
        store.update_address(
            &fake_peer,
            &addr2,
            crate::store::AddressSource::Manual,
            false,
        );
        assert!(
            *store
                .address_book
                .get(&fake_peer)
                .expect("peer to be in the store")
                .records()
                .last()
                .expect("addr in the record")
                .address
                == addr1
        );
        store.update_address(
            &fake_peer,
            &addr1,
            crate::store::AddressSource::Manual,
            false,
        );
        assert!(
            *store
                .address_book
                .get(&fake_peer)
                .expect("peer to be in the store")
                .records()
                .last()
                .expect("addr in the record")
                .address
                == addr2
        );
    }

    #[test]
    fn bounded_store() {
        let mut store = MemoryStore::new(Default::default());
        let fake_peer = PeerId::random();
        for i in 1..10 {
            let addr_string = format!("/ip4/127.0.0.{}", i);
            store.update_address(
                &fake_peer,
                &Multiaddr::from_str(&addr_string).expect("parsing to succeed"),
                crate::store::AddressSource::Manual,
                false,
            );
        }
        let first_record = Multiaddr::from_str("/ip4/127.0.0.1").expect("parsing to succeed");
        assert!(!store
            .addresses_of_peer(&fake_peer)
            .expect("peer to be in the store")
            .any(|addr| *addr == first_record));
        let second_record = Multiaddr::from_str("/ip4/127.0.0.2").expect("parsing to succeed");
        assert!(store
            .addresses_of_peer(&fake_peer)
            .expect("peer to be in the store")
            .any(|addr| *addr == second_record));
    }
}