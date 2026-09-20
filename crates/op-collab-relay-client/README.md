# op-collab-relay-client

`op-collab-relay-client` is OpenPencil's open-source, native relay data-plane
client. It adapts the existing loopback TCP collaboration transport to a
public WebSocket relay without terminating or inspecting the inner Noise
session.

The bridge is deliberately narrow:

- production endpoints must use `wss://`;
- unencrypted `ws://` is accepted only for numeric loopback endpoints in
  debug builds and tests;
- strict authentication is supplied by a caller-owned `RelayAuthenticator`;
- every connection phase, frame, byte count, and lifetime is bounded;
- only binary application messages are accepted;
- locator, route capability, credential, endpoint, and payload values are
  never included in `Debug` output or errors.

The owner selects the relay endpoint. A China-hosted owner may therefore
anchor the session at a China relay while overseas guests connect to that
same endpoint. The client does not silently migrate a session based on the
guest's current region.

The bearer credential admits relay bandwidth only. It is not a document
authorization grant: the inner Noise handshake, signed collaboration ticket,
and the desktop's owner-key/session/generation checks remain the document
permission boundary.

`RelayAuthenticator::begin_attempt` returns a non-cloneable attempt that owns
one fresh bearer. That exact bearer is placed on the WebSocket upgrade request,
retained while the `101` challenge is parsed, and bound into the dynamically
generated mode-2 hello proof. Owner lane replenishment and every reconnect
begin a new attempt, so ticket refresh cannot mix a new bearer with an old
proof.

`RelayClientX25519Agreement` exposes only the existing device public key and a
zeroizing X25519 agreement operation. It never exports the device private key.
`PinnedRelayX25519Keys` selects the relay public key by the challenge key id.

Strict `start` APIs require this challenge-bound authenticator and fail closed
when the response challenge is absent, duplicated, malformed, or selects an
unknown pin. The separately named `start_ticket_binding_only` APIs are the
only reduced-assurance compatibility path; they never silently fall back from
mode 2.

`RelayCredential` accepts the RFC 6750 `b64token` character set and caps the
bearer itself at 48 KiB; the complete Authorization value adds the 7-byte
`Bearer ` prefix. `stop` cancels and joins the worker; dropping a bridge
cancels and aborts it synchronously.

This crate contains no signing key, service credential, account policy, or
relay-server implementation.
