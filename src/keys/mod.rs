// Copyright 2019 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// https://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

//! Module providing keys, keypairs, and signatures.
//!
//! The easiest way to get a `PublicKey` is to create a random `Keypair` first through one of the
//! `new` functions. A `PublicKey` can't be generated by itself; it must always be derived from a
//! secret key.

mod proof;

use crate::{utils, Error, Result};
use hex_fmt::HexFmt;
use multibase::Decodable;
pub use proof::{BlsProof, BlsProofShare, Ed25519Proof, Proof, Proven};
use rand::{CryptoRng, Rng};
use serde::{Deserialize, Serialize};
use signature::{Signer, Verifier};
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
};
use threshold_crypto::{self, serde_impl::SerdeSecret};
use unwrap::unwrap;
use xor_name::{XorName, XOR_NAME_LEN};

/// Wrapper for different public key types.
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum PublicKey {
    /// Ed25519 public key.
    Ed25519(ed25519_dalek::PublicKey),
    /// BLS public key.
    Bls(threshold_crypto::PublicKey),
    /// BLS public key share.
    BlsShare(threshold_crypto::PublicKeyShare),
}

impl PublicKey {
    /// Returns the ed25519 key, if applicable.
    pub fn ed25519(&self) -> Option<ed25519_dalek::PublicKey> {
        if let Self::Ed25519(key) = self {
            Some(*key)
        } else {
            None
        }
    }

    /// Returns the BLS key, if applicable.
    pub fn bls(&self) -> Option<threshold_crypto::PublicKey> {
        if let Self::Bls(key) = self {
            Some(*key)
        } else {
            None
        }
    }

    /// Returns the BLS key share, if applicable.
    pub fn bls_share(&self) -> Option<threshold_crypto::PublicKeyShare> {
        if let Self::BlsShare(key) = self {
            Some(*key)
        } else {
            None
        }
    }

    /// Returns `Ok(())` if `signature` matches the message and `Err(Error::InvalidSignature)`
    /// otherwise.
    pub fn verify<T: AsRef<[u8]>>(&self, signature: &Signature, data: T) -> Result<()> {
        let is_valid = match (self, signature) {
            (Self::Ed25519(pub_key), Signature::Ed25519(sig)) => {
                pub_key.verify(data.as_ref(), sig).is_ok()
            }
            (Self::Bls(pub_key), Signature::Bls(sig)) => pub_key.verify(sig, data),
            (Self::BlsShare(pub_key), Signature::BlsShare(sig)) => pub_key.verify(&sig.share, data),
            _ => return Err(Error::SigningKeyTypeMismatch),
        };
        if is_valid {
            Ok(())
        } else {
            Err(Error::InvalidSignature)
        }
    }

    /// Returns the `PublicKey` serialised and encoded in z-base-32.
    pub fn encode_to_zbase32(&self) -> String {
        utils::encode(&self)
    }

    /// Creates from z-base-32 encoded string.
    pub fn decode_from_zbase32<I: Decodable>(encoded: I) -> Result<Self> {
        utils::decode(encoded)
    }
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for PublicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        utils::serialise(&self).hash(state)
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &PublicKey) -> Ordering {
        utils::serialise(&self).cmp(&utils::serialise(other))
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &PublicKey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<PublicKey> for XorName {
    fn from(public_key: PublicKey) -> Self {
        let bytes = match public_key {
            PublicKey::Ed25519(pub_key) => {
                return XorName(pub_key.to_bytes());
            }
            PublicKey::Bls(pub_key) => pub_key.to_bytes(),
            PublicKey::BlsShare(pub_key) => pub_key.to_bytes(),
        };
        let mut xor_name = XorName::random();
        xor_name.0.clone_from_slice(&bytes[..XOR_NAME_LEN]);
        xor_name
    }
}

impl From<ed25519_dalek::PublicKey> for PublicKey {
    fn from(public_key: ed25519_dalek::PublicKey) -> Self {
        Self::Ed25519(public_key)
    }
}

impl From<threshold_crypto::PublicKey> for PublicKey {
    fn from(public_key: threshold_crypto::PublicKey) -> Self {
        Self::Bls(public_key)
    }
}

impl From<threshold_crypto::PublicKeyShare> for PublicKey {
    fn from(public_key: threshold_crypto::PublicKeyShare) -> Self {
        Self::BlsShare(public_key)
    }
}

impl From<&Keypair> for PublicKey {
    fn from(keypair: &Keypair) -> Self {
        keypair.public_key()
    }
}

impl Debug for PublicKey {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "PublicKey::")?;
        match self {
            Self::Ed25519(pub_key) => {
                write!(formatter, "Ed25519({:<8})", HexFmt(&pub_key.to_bytes()))
            }
            Self::Bls(pub_key) => write!(
                formatter,
                "Bls({:<8})",
                HexFmt(&pub_key.to_bytes()[..XOR_NAME_LEN])
            ),
            Self::BlsShare(pub_key) => write!(
                formatter,
                "BlsShare({:<8})",
                HexFmt(&pub_key.to_bytes()[..XOR_NAME_LEN])
            ),
        }
    }
}

impl Display for PublicKey {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, formatter)
    }
}

/// A signature share, with its index in the combined collection.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Serialize, Deserialize, Debug)]
pub struct SignatureShare {
    /// Index in the combined collection.
    pub index: usize,
    /// Signature over some data.
    pub share: threshold_crypto::SignatureShare,
}

/// Wrapper for different signature types.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Signature {
    /// Ed25519 signature.
    Ed25519(ed25519_dalek::Signature),
    /// BLS signature.
    Bls(threshold_crypto::Signature),
    /// BLS signature share.
    BlsShare(SignatureShare),
}

impl Signature {
    /// Returns threshold_crypto::Signature if Self is a BLS variant.
    pub fn into_bls(self) -> Option<threshold_crypto::Signature> {
        match self {
            Self::Bls(sig) => Some(sig),
            _ => None,
        }
    }
}

impl From<threshold_crypto::Signature> for Signature {
    fn from(sig: threshold_crypto::Signature) -> Self {
        Self::Bls(sig)
    }
}

impl From<ed25519_dalek::Signature> for Signature {
    fn from(sig: ed25519_dalek::Signature) -> Self {
        Self::Ed25519(sig)
    }
}

impl From<SignatureShare> for Signature {
    fn from(sig: SignatureShare) -> Self {
        Self::BlsShare(sig)
    }
}

impl From<(usize, threshold_crypto::SignatureShare)> for Signature {
    fn from(sig: (usize, threshold_crypto::SignatureShare)) -> Self {
        let (index, share) = sig;
        Self::BlsShare(SignatureShare { index, share })
    }
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Signature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        utils::serialise(&self).hash(state)
    }
}

impl Ord for Signature {
    fn cmp(&self, other: &Signature) -> Ordering {
        utils::serialise(&self).cmp(&utils::serialise(other))
    }
}

impl PartialOrd for Signature {
    fn partial_cmp(&self, other: &Signature) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Debug for Signature {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Signature::")?;
        match self {
            Self::Ed25519(_) => write!(formatter, "Ed25519(..)"),
            Self::Bls(_) => write!(formatter, "Bls(..)"),
            Self::BlsShare(_) => write!(formatter, "BlsShare(..)"),
        }
    }
}

/// Wrapper for different keypair types.
#[derive(Serialize, Deserialize)]
pub enum Keypair {
    /// Ed25519 keypair.
    Ed25519(ed25519_dalek::Keypair),
    /// BLS keypair.
    Bls(BlsKeypair),
    /// BLS keypair share.
    BlsShare(BlsKeypairShare),
}

// Need to manually implement this due to a missing impl in `Ed25519::Keypair`.
impl Clone for Keypair {
    fn clone(&self) -> Self {
        match self {
            Self::Ed25519(keypair) => Self::Ed25519(unwrap!(ed25519_dalek::Keypair::from_bytes(
                &keypair.to_bytes()
            ))),
            Self::Bls(keypair) => Self::Bls(keypair.clone()),
            Self::BlsShare(keypair) => Self::BlsShare(keypair.clone()),
        }
    }
}

impl Debug for Keypair {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Keypair::")?;
        match self {
            Self::Ed25519(_) => write!(formatter, "Ed25519(..)"),
            Self::Bls(_) => write!(formatter, "Bls(..)"),
            Self::BlsShare(_) => write!(formatter, "BlsShare(..)"),
        }
    }
}

// Need to manually implement this due to a missing impl in `Ed25519::Keypair`.
impl PartialEq for Keypair {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ed25519(keypair), Self::Ed25519(other_keypair)) => {
                // TODO: After const generics land, remove the `to_vec()` calls.
                keypair.to_bytes().to_vec() == other_keypair.to_bytes().to_vec()
            }
            (Self::Bls(keypair), Self::Bls(other_keypair)) => keypair == other_keypair,
            (Self::BlsShare(keypair), Self::BlsShare(other_keypair)) => keypair == other_keypair,
            _ => false,
        }
    }
}

// Need to manually implement this due to a missing impl in `Ed25519::Keypair`.
impl Eq for Keypair {}

impl Keypair {
    /// Constructs a random Ed25519 public keypair.
    pub fn new_ed25519<T: CryptoRng + Rng>(rng: &mut T) -> Self {
        let keypair = ed25519_dalek::Keypair::generate(rng);
        Self::Ed25519(keypair)
    }

    /// Constructs a random BLS public keypair.
    pub fn new_bls<T: CryptoRng + Rng>(rng: &mut T) -> Self {
        let bls_secret_key: threshold_crypto::SecretKey = rng.gen();
        let bls_public_key = bls_secret_key.public_key();
        let keypair = BlsKeypair {
            secret: SerdeSecret(bls_secret_key),
            public: bls_public_key,
        };
        Self::Bls(keypair)
    }

    /// Constructs a BLS public keypair share.
    pub fn new_bls_share(
        index: usize,
        secret_share: threshold_crypto::SecretKeyShare,
        public_key_set: threshold_crypto::PublicKeySet,
    ) -> Self {
        let public_share = secret_share.public_key_share();
        let keypair_share = BlsKeypairShare {
            index,
            secret: SerdeSecret(secret_share),
            public: public_share,
            public_key_set,
        };
        Self::BlsShare(keypair_share)
    }

    /// Returns the public key associated with this keypair.
    pub fn public_key(&self) -> PublicKey {
        match self {
            Self::Ed25519(keypair) => PublicKey::Ed25519(keypair.public),
            Self::Bls(keypair) => PublicKey::Bls(keypair.public),
            Self::BlsShare(keypair) => PublicKey::BlsShare(keypair.public),
        }
    }

    /// Signs with the underlying keypair.
    pub fn sign(&self, data: &[u8]) -> Signature {
        match self {
            Self::Ed25519(keypair) => Signature::Ed25519(keypair.sign(&data)),
            Self::Bls(keypair) => Signature::Bls(keypair.secret.sign(data)),
            Self::BlsShare(keypair) => {
                let index = keypair.index;
                let share = keypair.secret.sign(data);
                Signature::BlsShare(SignatureShare { index, share })
            }
        }
    }
}

/// BLS keypair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlsKeypair {
    /// Secret key.
    pub secret: SerdeSecret<threshold_crypto::SecretKey>,
    /// Public key.
    pub public: threshold_crypto::PublicKey,
}

/// BLS keypair share.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlsKeypairShare {
    /// Share index.
    pub index: usize,
    /// Secret key share.
    pub secret: SerdeSecret<threshold_crypto::SecretKeyShare>,
    /// Public key share.
    pub public: threshold_crypto::PublicKeyShare,
    /// Public key set. Necessary for producing proofs.
    pub public_key_set: threshold_crypto::PublicKeySet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils;
    use bincode::deserialize as deserialise;
    use threshold_crypto::{self};

    fn gen_keypairs() -> Vec<Keypair> {
        let mut rng = rand::thread_rng();
        let bls_secret_key = threshold_crypto::SecretKeySet::random(1, &mut rng);
        vec![
            Keypair::new_ed25519(&mut rng),
            Keypair::new_bls(&mut rng),
            Keypair::new_bls_share(
                0,
                bls_secret_key.secret_key_share(0),
                bls_secret_key.public_keys(),
            ),
        ]
    }

    fn gen_keys() -> Vec<PublicKey> {
        gen_keypairs().iter().map(PublicKey::from).collect()
    }

    #[test]
    fn zbase32_encode_decode_public_key() {
        use unwrap::unwrap;

        let keys = gen_keys();

        for key in keys {
            assert_eq!(
                key,
                unwrap!(PublicKey::decode_from_zbase32(&key.encode_to_zbase32()))
            );
        }
    }

    // Test serialising and deserialising public keys.
    #[test]
    fn serialisation_public_key() {
        let keys = gen_keys();

        for key in keys {
            let encoded = utils::serialise(&key);
            let decoded: PublicKey = unwrap!(deserialise(&encoded));

            assert_eq!(decoded, key);
        }
    }

    // Test serialising and deserialising key pairs.
    #[test]
    fn serialisation_key_pair() {
        let keypairs = gen_keypairs();

        for keypair in keypairs {
            let encoded = utils::serialise(&keypair);
            let decoded: Keypair = unwrap!(deserialise(&encoded));

            assert_eq!(decoded, keypair);
        }
    }
}
