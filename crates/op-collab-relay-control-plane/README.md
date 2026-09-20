# OpenPencil relay locator control plane

This MIT-licensed crate implements the production locator-issuance core without
shipping a production private key.

The owner-side flow is deliberately split:

1. `OwnerPublishDraft` generates a fresh route id, generation, and 32-byte
   bearer capability with the operating-system CSPRNG.
2. Its fixed-size `OwnerPublishRequest` carries only the home region, public
   route metadata, owner Noise public key, stable relay discovery id, and a
   requested lifetime of at most one hour. The capability is not encoded or
   uploaded.
3. The control plane verifies an opaque collaboration ticket with
   `op-auth-bridge`, passes only `VerifiedCollabClaims` into
   `TicketVerifiedOwnerBinding`, and rejects unless the ticket-bound device DH
   public key equals the requested owner Noise public key.
4. `RelayLocatorIssuer` builds bounded claims and delegates the canonical
   signature to an external `RelayLocatorSigner`. Production deployments
   implement that trait with HSM/KMS-backed Ed25519. This repository contains
   no production signer or private key.
5. The owner decodes the bounded response, verifies it with a pinned
   `RelayLocatorVerifier`, checks every request-bound claim, and only then
   combines the signed locator with its local capability into the `opc1_`
   invite.

`RelayLocatorPublishService` is the transport-neutral `POST /v1/locator`
handler core. It single-sources ticket verification and requires an injected
`RelayOwnerPublishPolicy`; `RegionBoundOwnerPublishPolicy` is the explicit
choice when every authenticated account may host in one configured residency.

A ticket must be current when a locator is issued, but locator expiry is not
clamped to that ticket's shorter expiry. The signed locator may remain usable
for the requested period (never more than one hour) so refreshed tickets can
reauthenticate and reconnect to the same invite. The relay still requires and
verifies a current DH-bound ticket on every tunnel authentication; a locator
signature never substitutes for ticket authorization.

`RelayLocatorHttpClient` is the concrete blocking client. Production endpoints
must be HTTPS with the exact path and no user info, query, or fragment. The
client disables redirects and implicit environment proxies, uses bounded
connect/total timeouts and request/response sizes, marks the Authorization
header sensitive, and never places the ticket, locator, or invite in a URL or
Debug output. A debug HTTP exception is compiled only for test/debug builds and
requires both an explicit opt-in and a numeric loopback host.

The deployable HTTP boundary and Unix HSM client bridge live in
`op-collab-relay-locator-server`; the desktop injects this crate's pinned
HTTPS client through its environment adapter. Production still requires an
operator-owned HSM-side adapter and non-exportable private key, public locator
key distribution/rotation, DNS/TLS, signed-policy provisioning, and field
deployment. No production private signing key is present in either crate.
