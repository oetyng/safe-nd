// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// https://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

//! SAFE network data types.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/maidsafe/QA/master/Images/maidsafe_logo.png",
    html_favicon_url = "https://maidsafe.net/img/favicon.ico",
    test(attr(forbid(warnings)))
)]
// For explanation of lint checks, run `rustc -W help`.
#![forbid(unsafe_code)]
#![warn(
    // TODO: add missing debug implementations for structs?
    // missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

mod blob;
mod errors;
mod identity;
mod keys;
mod map;
mod messaging;
mod money;
mod rewards;
mod sequence;
mod transfer;
mod utils;

pub use blob::{
    Address as BlobAddress, Data as Blob, Kind as BlobKind, PrivateData as PrivateBlob,
    PublicData as PublicBlob, MAX_BLOB_SIZE_IN_BYTES,
};
pub use errors::{EntryError, Error, Result};
pub use identity::{
    app::{FullId as AppFullId, PublicId as AppPublicId},
    client::{FullId as ClientFullId, PublicId as ClientPublicId},
    node::{FullId as NodeFullId, PublicId as NodePublicId},
    node_keypairs::{NodeKeypairs, PublicKeys},
    PublicId, SafeKey,
};
pub use keys::{
    BlsKeypair, BlsKeypairShare, BlsProof, BlsProofShare, Ed25519Proof, Keypair, Proof, Proven,
    PublicKey, Signature, SignatureShare,
};
pub use map::{
    Action as MapAction, Address as MapAddress, Data as Map, Entries as MapEntries,
    EntryActions as MapEntryActions, Kind as MapKind, PermissionSet as MapPermissionSet,
    SeqData as SeqMap, SeqEntries as MapSeqEntries, SeqEntryAction as MapSeqEntryAction,
    SeqEntryActions as MapSeqEntryActions, SeqValue as MapSeqValue, UnseqData as UnseqMap,
    UnseqEntries as MapUnseqEntries, UnseqEntryAction as MapUnseqEntryAction,
    UnseqEntryActions as MapUnseqEntryActions, Value as MapValue, Values as MapValues,
};
pub use messaging::*;
pub use money::Money;
pub use rewards::{RewardCounter, Work};

pub use sequence::{
    Action as SequenceAction, Address as SequenceAddress, Data as Sequence,
    Entries as SequenceEntries, Entry as SequenceEntry, Index as SequenceIndex,
    Indices as SequenceIndices, Kind as SequenceKind, Owner as SequenceOwner,
    Permissions as SequencePermissions, PrivSeqData,
    PrivUserPermissions as SequencePrivUserPermissions,
    PrivatePermissions as SequencePrivatePermissions, PubSeqData,
    PubUserPermissions as SequencePubUserPermissions,
    PublicPermissions as SequencePublicPermissions, User as SequenceUser,
    UserPermissions as SequenceUserPermissions, WriteOp as SequenceWriteOp,
};
pub use sha3::Sha3_512 as Ed25519Digest;
pub use transfer::*;
pub use utils::verify_signature;

use hex_fmt::HexFmt;
use multibase::Decodable;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Display, Formatter},
    net::SocketAddr,
};

/// Object storing a data variant.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub enum Data {
    /// Blob.
    Immutable(Blob),
    /// MutableData.
    Mutable(Map),
    /// Sequence.
    Sequence(Sequence),
}

impl Data {
    /// Returns true if published.
    pub fn is_pub(&self) -> bool {
        match *self {
            Self::Immutable(ref idata) => idata.is_pub(),
            Self::Mutable(_) => false,
            Self::Sequence(ref sequence) => sequence.is_pub(),
        }
    }

    /// Returns true if unpublished.
    pub fn is_unpub(&self) -> bool {
        !self.is_pub()
    }
}

impl From<Blob> for Data {
    fn from(data: Blob) -> Self {
        Self::Immutable(data)
    }
}

impl From<Map> for Data {
    fn from(data: Map) -> Self {
        Self::Mutable(data)
    }
}

impl From<Sequence> for Data {
    fn from(data: Sequence) -> Self {
        Self::Sequence(data)
    }
}

/// Permissions for an app stored by the Client Handlers.
#[derive(
    Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Clone, Serialize, Deserialize, Default, Debug,
)]
pub struct AppPermissions {
    /// Whether this app has permissions to perform data mutations.
    pub data_mutations: bool,
    /// Whether this app has permissions to transfer money.
    pub transfer_money: bool,
    /// Whether this app has permissions to read the account balance.
    pub read_balance: bool,
    /// Whether this app has permissions to read the account transfer history.
    pub read_transfer_history: bool,
}

/// Constant byte length of `XorName`.
pub const XOR_NAME_LEN: usize = 32;

/// A [`XOR_NAME_BITS`](constant.XOR_NAME_BITS.html)-bit number, viewed as a point in XOR space.
///
/// This wraps an array of [`XOR_NAME_LEN`](constant.XOR_NAME_LEN.html) bytes, i.e. a number
/// between 0 and 2<sup>`XOR_NAME_BITS`</sup> - 1.
///
/// XOR space is the space of these numbers, with the [XOR metric][1] as a notion of distance,
/// i. e. the points with IDs `x` and `y` are considered to have distance `x xor y`.
///
/// [1]: https://en.wikipedia.org/wiki/Kademlia#System_details
#[derive(Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct XorName(pub [u8; XOR_NAME_LEN]);

impl XorName {
    /// Returns the `XorName` serialised and encoded in z-base-32.
    pub fn encode_to_zbase32(&self) -> String {
        utils::encode(&self)
    }

    /// Creates from z-base-32 encoded string.
    pub fn decode_from_zbase32<I: Decodable>(encoded: I) -> Result<Self> {
        utils::decode(encoded)
    }
}

impl Debug for XorName {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{:<8}", HexFmt(&self.0))
    }
}

impl Display for XorName {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, formatter)
    }
}

impl Distribution<XorName> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> XorName {
        XorName(rng.gen())
    }
}

/// Handshake requests sent from clients to vaults to establish new connections and verify a client's
/// key (to prevent replay attacks).
#[derive(Serialize, Deserialize)]
pub enum HandshakeRequest {
    /// Sent by clients as an initial bootstrap request, and then for subsequent bootstrap attempts.
    Bootstrap(PublicId),
    /// Sent to destination nodes as a response to `HandshakeResponse::Join`.
    Join(PublicId),
    /// Response to `HandshakeResponse::Challenge` sent by a vault.
    ChallengeResult(Signature),
}

/// Handshake responses sent from vaults to clients.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize)]
pub enum HandshakeResponse {
    /// Sent by nodes when a client should attempt to connect to the section that's closest to
    /// its destination (section managing the client's account).
    Rebootstrap(Vec<(XorName, SocketAddr)>),
    /// Sent by nodes when a client reaches its destination section.
    Join(Vec<(XorName, SocketAddr)>),
    /// Sent by nodes as a response to a valid `HandshakeRequest::Join`.
    Challenge(PublicId, Vec<u8>),
    /// Sent by nodes as a response to an invalid `HandshakeRequest::Join` (when a client attempts to join a wrong section).
    InvalidSection,
}

#[cfg(test)]
mod tests {
    use crate::XorName;
    use unwrap::unwrap;

    #[test]
    fn zbase32_encode_decode_xorname() {
        let name = XorName(rand::random());
        let encoded = name.encode_to_zbase32();
        let decoded = unwrap!(XorName::decode_from_zbase32(&encoded));
        assert_eq!(name, decoded);
    }
}
