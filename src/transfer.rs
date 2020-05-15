use super::keys::{PublicKey, Signature};
use super::money::Money;
use crdts::Dot;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use threshold_crypto;

/// Transfer ID.
pub type TransferId = Dot<PublicKey>;

/// Op
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct Transfer {
    /// Transfer ID.
    pub id: TransferId,
    /// The destination to transfer to.
    pub to: PublicKey,
    /// The amount to transfer.
    pub amount: Money,
    /// Determines the behaviour of a Transfer.
    pub restrictions: TransferRestrictions,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
/// Determines the behaviour of a Transfer
/// when validation over the different possible states is done.
pub enum TransferRestrictions {
    /// Fails transfer if the key has no history.
    RequireHistory,
    /// Fails transfer if there are previously recorded transfers.
    ExpectNoHistory,
    /// Transfers regardless of previous history.
    NoRestriction,
}

/// A Client cmd.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct ValidateTransfer {
    /// The transfer.
    pub transfer: Transfer,
    /// Client signature over the transfer.
    pub client_signature: Signature,
}

/// The Elder event raised when
/// ValidateTransfer cmd has been successful.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct TransferValidated {
    /// The cmd generated by client.
    pub transfer_cmd: ValidateTransfer,
    /// Elder signature over the transfer cmd.
    pub elder_signature: threshold_crypto::SignatureShare,
    // /// The PK Set of the section
    // pub pk_set: threshold_crypto::PublicKeySet, // temporary commented out
}

/// A Client cmd.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct RegisterTransfer {
    /// The transfer proof.
    pub proof: ProofOfAgreement,
}

/// The Elder event raised when
/// RegisterTransfer cmd has been successful.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct TransferRegistered {
    /// The transfer proof.
    pub proof: ProofOfAgreement,
}

/// The aggregated Elder signatures of the client transfer cmd.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct ProofOfAgreement {
    /// The cmd generated by client.
    pub transfer_cmd: ValidateTransfer,
    /// Quorum of Elder sigs over the transfer cmd.
    pub section_sig: Signature,
}

// /// (Draft) A Client cmd to roll back a failed transfer.
// #[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
// pub struct CancelTransfer {
//     /// The transfer id.
//     pub transfer_id: TransferId,
//     /// Client signature over the transfer id.
//     pub client_signature: Signature,
// }

/// Notification of a Transfer sent to a recipient.
#[derive(Hash, Eq, PartialEq, PartialOrd, Ord, Clone, Serialize, Deserialize, Debug)]
pub struct TransferNotification(pub ProofOfAgreement);