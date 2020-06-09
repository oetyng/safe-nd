// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// https://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use super::{AuthorisationKind, MiscAuthKind, MoneyAuthKind, Type};
use crate::{DebitAgreementProof, Error, Response, SignedTransfer, XorName, PublicKey};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt};

/// Money request that is sent to Elders.
#[derive(Hash, Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum MoneyRequest {
    // ===== Money =====
    //
    /// Request to validate transfer.
    ValidateTransfer {
        /// The cmd to validate a transfer.
        signed_transfer: SignedTransfer,
    },
    /// Request to register transfer.
    RegisterTransfer {
        /// The cmd to register the consensused transfer.
        proof: DebitAgreementProof,
    },
    /// Request to propagate transfer.
    PropagateTransfer {
        /// The cmd to register the consensused transfer.
        proof: DebitAgreementProof,
    },
    /// Get key balance.
    GetBalance(PublicKey),
    /// Get key transfers since specified version.
    GetHistory {
        /// The balance key.
        at: PublicKey,
        /// The last version of transfers we know of.
        since_version: usize,
    },
}

impl MoneyRequest {
    /// Get the `Type` of this `Request`.
    pub fn get_type(&self) -> Type {
        use MoneyRequest::*;
        match *self {
            GetBalance(_) => Type::PrivateGet,
            GetHistory { .. } => Type::PrivateGet,
            ValidateTransfer { .. } => Type::Transfer, // TODO: fix..
            RegisterTransfer { .. } => Type::Transfer, // TODO: fix..
            PropagateTransfer { .. } => Type::Transfer, // TODO: fix..
        }
    }

    /// Creates a Response containing an error, with the Response variant corresponding to the
    /// Request variant.
    pub fn error_response(&self, error: Error) -> Response {
        use MoneyRequest::*;
        match *self {
            GetBalance(_) => Response::GetBalance(Err(error)),
            GetHistory { .. } => Response::GetHistory(Err(error)),
            ValidateTransfer { .. } => Response::TransferValidation(Err(error)),
            RegisterTransfer { .. } => Response::TransferRegistration(Err(error)),
            PropagateTransfer { .. } => Response::TransferPropagation(Err(error)),
        }
    }

    /// Returns the type of authorisation needed for the request.
    pub fn authorisation_kind(&self) -> AuthorisationKind {
        use MoneyRequest::*;
        match self.clone() {
            PropagateTransfer { .. } => AuthorisationKind::None, // the proof has the authority within it
            RegisterTransfer { .. } => AuthorisationKind::None, // the proof has the authority within it
            ValidateTransfer { .. } => AuthorisationKind::Misc(MiscAuthKind::MutAndTransferMoney),
            GetBalance(_) => AuthorisationKind::Money(MoneyAuthKind::GetBalance), // current state
            GetHistory { .. } => AuthorisationKind::Money(MoneyAuthKind::GetHistory), // history of incoming transfers
        }
    }

    /// Returns the address of the destination for `request`.
    pub fn dest_address(&self) -> Option<Cow<XorName>> {
        use MoneyRequest::*;
        match self {
            PropagateTransfer { ref proof, .. } => Some(Cow::Owned(XorName::from(
                proof.signed_transfer.transfer.to, // sent to section where credit is made
            ))),
            RegisterTransfer { ref proof, .. } => Some(Cow::Owned(XorName::from(
                proof.signed_transfer.transfer.id.actor, // this is handled where the debit is made
            ))),
            ValidateTransfer {
                ref signed_transfer,
                ..
            } => {
                Some(Cow::Owned(XorName::from(signed_transfer.transfer.id.actor)))
                // this is handled where the debit is made
            }
            GetBalance(_) => None,
            GetHistory { .. } => None,
        }
    }
}

impl fmt::Debug for MoneyRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        use MoneyRequest::*;
        write!(
            formatter,
            "MoneyRequest::{}",
            match *self {
                PropagateTransfer { .. } => "PropagateTransfer",
                RegisterTransfer { .. } => "RegisterTransfer",
                ValidateTransfer { .. } => "ValidateTransfer",
                GetBalance(_) => "GetBalance",
                GetHistory { .. } => "GetHistory",
            }
        )
    }
}
