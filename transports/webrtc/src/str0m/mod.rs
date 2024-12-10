pub struct Rtc{
    inner: str0m::Rtc,
}


impl libp2p_core::Transport for Rtc{
    type Output;

    type Error = str0m::RtcError;

    type ListenerUpgrade;

    type Dial;

    fn listen_on(
        &mut self,
        id: libp2p_core::transport::ListenerId,
        addr: libp2p_core::Multiaddr,
    ) -> Result<(), libp2p_core::transport::TransportError<Self::Error>> {
        todo!()
    }

    fn remove_listener(&mut self, id: libp2p_core::transport::ListenerId) -> bool {
        todo!()
    }

    fn dial(
        &mut self,
        addr: libp2p_core::Multiaddr,
        opts: libp2p_core::transport::DialOpts,
    ) -> Result<Self::Dial, libp2p_core::transport::TransportError<Self::Error>> {
        todo!()
    }

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p_core::transport::TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        todo!()
    }
}

pub struct Connection{
    
}