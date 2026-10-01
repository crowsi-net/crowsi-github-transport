use crowsi_provider_egress_contracts::{GitHubEgressCommandV1, authorization_digest};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::{GitHubTransportTrustV1, TransportError};

pub(super) fn verify(
    value: &GitHubEgressCommandV1,
    trust: &GitHubTransportTrustV1,
) -> Result<String, TransportError> {
    let auth = &value.authorization;
    if auth.issuer != trust.issuer
        || auth.audience != trust.audience
        || auth.key_id != trust.key_id
        || auth.issued_at_epoch_s < trust.minimum_issued_at_epoch_s
    {
        return Err(TransportError("authorization-trust"));
    }
    let public: [u8; 32] = hex::decode(&trust.public_key_hex)
        .map_err(|_| TransportError("key"))?
        .try_into()
        .map_err(|_| TransportError("key"))?;
    let signature: [u8; 64] = hex::decode(&auth.signature_hex)
        .map_err(|_| TransportError("signature"))?
        .try_into()
        .map_err(|_| TransportError("signature"))?;
    let digest = authorization_digest(auth).map_err(TransportError)?;
    VerifyingKey::from_bytes(&public)
        .map_err(|_| TransportError("key"))?
        .verify(digest.as_bytes(), &Signature::from_bytes(&signature))
        .map_err(|_| TransportError("signature"))?;
    Ok(digest)
}
