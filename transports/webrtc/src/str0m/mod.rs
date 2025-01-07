use libp2p_core::PeerId;

mod sdp;

pub struct Rtc {
    inner: str0m::Rtc,
    listeners: SelectAll<ListenStream>,
    config: crate::tokio::transport::Config,
}

impl libp2p_core::Transport for Rtc {
    type Output = (PeerId, Connection);

    type Error = str0m::RtcError;

    type ListenerUpgrade = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    type Dial = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn listen_on(
        &mut self,
        id: libp2p_core::transport::ListenerId,
        addr: libp2p_core::Multiaddr,
    ) -> Result<(), libp2p_core::transport::TransportError<Self::Error>> {
        let socket_addr =
            parse_webrtc_listen_addr(&addr).ok_or(TransportError::MultiaddrNotSupported(addr))?;
        let udp_mux = UDPMuxNewAddr::listen_on(socket_addr)
            .map_err(|io| TransportError::Other(Error::Io(io)))?;
        self.listeners.push(
            ListenStream::new(id, self.config.clone(), udp_mux)
                .map_err(|e| TransportError::Other(Error::Io(e)))?,
        );

        Ok(())
    }

    fn remove_listener(&mut self, id: libp2p_core::transport::ListenerId) -> bool {
        if let Some(listener) = self.listeners.iter_mut().find(|l| l.listener_id == id) {
            listener.close(Ok(()));
            true
        } else {
            false
        }
    }

    fn dial(
        &mut self,
        addr: libp2p_core::Multiaddr,
        opts: libp2p_core::transport::DialOpts,
    ) -> Result<Self::Dial, libp2p_core::transport::TransportError<Self::Error>> {
        if dial_opts.role.is_listener() {
            // TODO: As the listener of a WebRTC hole punch, we need to send a random UDP packet to
            // the `addr`. See DCUtR specification below.
            //
            // https://github.com/libp2p/specs/blob/master/relay/DCUtR.md#the-protocol
            tracing::warn!("WebRTC hole punch is not yet supported");
        }

        let (sock_addr, server_fingerprint) = libp2p_webrtc_utils::parse_webrtc_dial_addr(&addr)
            .ok_or_else(|| TransportError::MultiaddrNotSupported(addr.clone()))?;
        if sock_addr.port() == 0 || sock_addr.ip().is_unspecified() {
            return Err(TransportError::MultiaddrNotSupported(addr));
        }
        let config = self.config.clone();
        let client_fingerprint = self.config.fingerprint;
        let udp_mux = self
            .listeners
            .iter()
            .next()
            .ok_or(TransportError::Other(Error::NoListeners))?
            .udp_mux
            .udp_mux_handle();
        todo!()
    }

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p_core::transport::TransportEvent<Self::ListenerUpgrade, Self::Error>>
    {
        todo!()
    }
}

pub struct Connection {}
