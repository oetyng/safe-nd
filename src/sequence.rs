// Copyright 2019 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// https://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use crate::permissions::{
    Permissions, PrivatePermissionSet, PrivatePermissions, PublicPermissionSet, PublicPermissions,
    Request,
};
use crate::shared_data::{
    to_absolute_index, to_absolute_range, Address, ExpectedIndices, Index, Kind, NonSentried,
    Owner, Sentried, User, Value,
};
use crate::{Error, PublicKey, Result, XorName};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Formatter};

pub type PublicSentriedSequence = Sequence<PublicPermissions, Sentried>;
pub type PublicSequence = Sequence<PublicPermissions, NonSentried>;
pub type PrivateSentriedSequence = Sequence<PrivatePermissions, Sentried>;
pub type PrivateSequence = Sequence<PrivatePermissions, NonSentried>;
pub type Values = Vec<Value>;

#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
pub enum SequencePermissions {
    Public(PublicPermissions),
    Private(PrivatePermissions),
}

impl From<PrivatePermissions> for SequencePermissions {
    fn from(permissions: PrivatePermissions) -> Self {
        SequencePermissions::Private(permissions)
    }
}

impl From<PublicPermissions> for SequencePermissions {
    fn from(permissions: PublicPermissions) -> Self {
        SequencePermissions::Public(permissions)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default, Debug)]
pub struct DataEntry {
    pub index: u64,
    pub value: Vec<u8>,
}

impl DataEntry {
    pub fn new(index: u64, value: Vec<u8>) -> Self {
        Self { index, value }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct Sequence<P, S> {
    address: Address,
    data: Values,
    permissions: Vec<P>,
    // This is the history of owners, with each entry representing an owner.  Each single owner
    // could represent an individual user, or a group of users, depending on the `PublicKey` type.
    owners: Vec<Owner>,
    _flavour: S,
}

/// Common methods for all `Sequence` flavours.
impl<P, S> Sequence<P, S>
where
    P: Permissions,
    S: Copy,
{
    /// Returns the data shell - that is - everything except the Values themselves.
    pub fn shell(&self, expected_data_index: impl Into<Index>) -> Result<Self> {
        let expected_data_index = to_absolute_index(
            expected_data_index.into(),
            self.expected_data_index() as usize,
        )
        .ok_or(Error::NoSuchEntry)? as u64;

        let permissions = self
            .permissions
            .iter()
            .filter(|perm| perm.expected_data_index() <= expected_data_index)
            .cloned()
            .collect();

        let owners = self
            .owners
            .iter()
            .filter(|owner| owner.expected_data_index <= expected_data_index)
            .cloned()
            .collect();

        Ok(Self {
            address: self.address,
            data: Vec::new(),
            permissions,
            owners,
            _flavour: self._flavour,
        })
    }

    /// Return a value for the given index (if it is present).
    pub fn get(&self, index: u64) -> Option<&Value> {
        self.data.get(index as usize)
    }

    /// Return the current data entry (if it is present).
    pub fn current_data_entry(&self) -> Option<DataEntry> {
        match self.data.last() {
            Some(value) => Some(DataEntry::new(self.data.len() as u64, value.to_vec())),
            None => None,
        }
    }

    /// Get a range of values within the given indices.
    pub fn in_range(&self, start: Index, end: Index) -> Option<Values> {
        let range = to_absolute_range(start, end, self.data.len())?;
        Some(self.data[range].to_vec())
    }

    /// Return all Values.
    pub fn values(&self) -> &Values {
        &self.data
    }

    /// Return the address of this Sequence.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Return the name of this Sequence.
    pub fn name(&self) -> &XorName {
        self.address.name()
    }

    /// Return the type tag of this Sequence.
    pub fn tag(&self) -> u64 {
        self.address.tag()
    }

    /// Return the expected data index.
    pub fn expected_data_index(&self) -> u64 {
        self.data.len() as u64
    }

    /// Return the expected owners index.
    pub fn expected_owners_index(&self) -> u64 {
        self.owners.len() as u64
    }

    /// Return the expected permissions index.
    pub fn expected_permissions_index(&self) -> u64 {
        self.permissions.len() as u64
    }

    /// Get history of permission within the range of indices specified.
    pub fn permission_history_range(&self, start: Index, end: Index) -> Option<&[P]> {
        let range = to_absolute_range(start, end, self.permissions.len())?;
        Some(&self.permissions[range])
    }

    /// Set permissions.
    /// The `Permissions` struct needs to contain the correct expected indices.
    pub fn set_permissions(&mut self, permissions: P, index: u64) -> Result<()> {
        if permissions.expected_data_index() != self.expected_data_index() {
            return Err(Error::InvalidSuccessor(self.expected_data_index()));
        }
        if permissions.expected_owners_index() != self.expected_owners_index() {
            return Err(Error::InvalidOwnersSuccessor(self.expected_owners_index()));
        }
        if self.expected_permissions_index() != index {
            return Err(Error::InvalidSuccessor(self.expected_permissions_index()));
        }
        self.permissions.push(permissions);
        Ok(())
    }

    /// Get permissions at index.
    pub fn permissions_at(&self, index: impl Into<Index>) -> Option<&P> {
        let index = to_absolute_index(index.into(), self.permissions.len())?;
        self.permissions.get(index)
    }

    pub fn is_permitted(&self, user: PublicKey, request: Request) -> bool {
        match self.owner_at(Index::FromEnd(1)) {
            Some(owner) => {
                if owner.public_key == user {
                    return true;
                }
            }
            None => (),
        }
        match self.permissions_at(Index::FromEnd(1)) {
            Some(permissions) => permissions.is_permitted(&user, &request),
            None => false,
        }
    }

    /// Get owner at index.
    pub fn owner_at(&self, index: impl Into<Index>) -> Option<&Owner> {
        let index = to_absolute_index(index.into(), self.owners.len())?;
        self.owners.get(index)
    }

    /// Get history of owners within the range of indices specified.
    pub fn owner_history_range(&self, start: Index, end: Index) -> Option<&[Owner]> {
        let range = to_absolute_range(start, end, self.owners.len())?;
        Some(&self.owners[range])
    }

    /// Set owner.
    pub fn set_owner(&mut self, owner: Owner, index: u64) -> Result<()> {
        if owner.expected_data_index != self.expected_data_index() {
            return Err(Error::InvalidSuccessor(self.expected_data_index()));
        }
        if owner.expected_permissions_index != self.expected_permissions_index() {
            return Err(Error::InvalidPermissionsSuccessor(
                self.expected_permissions_index(),
            ));
        }
        if self.expected_owners_index() != index {
            return Err(Error::InvalidSuccessor(self.expected_owners_index()));
        }
        self.owners.push(owner);
        Ok(())
    }

    /// Returns true if the user is the current owner.
    pub fn is_owner(&self, user: PublicKey) -> bool {
        match self.owner_at(Index::FromEnd(1)) {
            Some(owner) => user == owner.public_key,
            _ => false,
        }
    }

    pub fn indices(&self) -> ExpectedIndices {
        ExpectedIndices::new(
            self.expected_data_index(),
            self.expected_owners_index(),
            self.expected_permissions_index(),
        )
    }
}

/// Common methods for NonSentried flavours.
impl<P: Permissions> Sequence<P, NonSentried> {
    /// Append new Values.
    pub fn append(&mut self, values: Values) -> Result<()> {
        self.data.extend(values);
        Ok(())
    }
}

/// Common methods for Sentried flavours.
impl<P: Permissions> Sequence<P, Sentried> {
    /// Append new Values.
    ///
    /// If the specified `expected_index` does not equal the Values count in data, an
    /// error will be returned.
    pub fn append(&mut self, values: Values, expected_index: u64) -> Result<()> {
        if expected_index != self.data.len() as u64 {
            return Err(Error::InvalidSuccessor(self.data.len() as u64));
        }

        self.data.extend(values);
        Ok(())
    }
}

/// Public + Sentried
impl Sequence<PublicPermissions, Sentried> {
    pub fn new(name: XorName, tag: u64) -> Self {
        Self {
            address: Address::PublicSentried { name, tag },
            data: Vec::new(),
            permissions: Vec::new(),
            owners: Vec::new(),
            _flavour: Sentried,
        }
    }
}

impl Debug for Sequence<PublicPermissions, Sentried> {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "PublicSentriedSequence {:?}", self.name())
    }
}

/// Public + NonSentried
impl Sequence<PublicPermissions, NonSentried> {
    pub fn new(name: XorName, tag: u64) -> Self {
        Self {
            address: Address::Public { name, tag },
            data: Vec::new(),
            permissions: Vec::new(),
            owners: Vec::new(),
            _flavour: NonSentried,
        }
    }
}

impl Debug for Sequence<PublicPermissions, NonSentried> {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "PublicSequence {:?}", self.name())
    }
}

/// Private + Sentried
impl Sequence<PrivatePermissions, Sentried> {
    pub fn new(name: XorName, tag: u64) -> Self {
        Self {
            address: Address::PrivateSentried { name, tag },
            data: Vec::new(),
            permissions: Vec::new(),
            owners: Vec::new(),
            _flavour: Sentried,
        }
    }
}

impl Debug for Sequence<PrivatePermissions, Sentried> {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "PrivateSentriedSequence {:?}", self.name())
    }
}

/// Private + NonSentried
impl Sequence<PrivatePermissions, NonSentried> {
    pub fn new(name: XorName, tag: u64) -> Self {
        Self {
            address: Address::Private { name, tag },
            data: Vec::new(),
            permissions: Vec::new(),
            owners: Vec::new(),
            _flavour: NonSentried,
        }
    }
}

impl Debug for Sequence<PrivatePermissions, NonSentried> {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "PrivateSequence {:?}", self.name())
    }
}

/// Object storing a Sequence variant.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub enum Data {
    PublicSentried(PublicSentriedSequence),
    Public(PublicSequence),
    PrivateSentried(PrivateSentriedSequence),
    Private(PrivateSequence),
}

impl Data {
    pub fn is_permitted(&self, request: Request, user: PublicKey) -> bool {
        match (self, request) {
            (Data::PublicSentried(_), Request::Query(_)) | (Data::Public(_), Request::Query(_)) => {
                return true
            }
            _ => (),
        }
        match self {
            Data::PublicSentried(data) => data.is_permitted(user, request),
            Data::Public(data) => data.is_permitted(user, request),
            Data::PrivateSentried(data) => data.is_permitted(user, request),
            Data::Private(data) => data.is_permitted(user, request),
        }
    }

    pub fn address(&self) -> &Address {
        match self {
            Data::PublicSentried(data) => data.address(),
            Data::Public(data) => data.address(),
            Data::PrivateSentried(data) => data.address(),
            Data::Private(data) => data.address(),
        }
    }

    pub fn kind(&self) -> Kind {
        self.address().kind()
    }

    pub fn name(&self) -> &XorName {
        self.address().name()
    }

    pub fn tag(&self) -> u64 {
        self.address().tag()
    }

    pub fn is_public(&self) -> bool {
        self.kind().is_public()
    }

    pub fn is_private(&self) -> bool {
        self.kind().is_private()
    }

    pub fn is_sentried(&self) -> bool {
        self.kind().is_sentried()
    }

    pub fn expected_data_index(&self) -> u64 {
        match self {
            Data::PublicSentried(data) => data.expected_data_index(),
            Data::Public(data) => data.expected_data_index(),
            Data::PrivateSentried(data) => data.expected_data_index(),
            Data::Private(data) => data.expected_data_index(),
        }
    }

    pub fn expected_permissions_index(&self) -> u64 {
        match self {
            Data::PublicSentried(data) => data.expected_permissions_index(),
            Data::Public(data) => data.expected_permissions_index(),
            Data::PrivateSentried(data) => data.expected_permissions_index(),
            Data::Private(data) => data.expected_permissions_index(),
        }
    }

    pub fn expected_owners_index(&self) -> u64 {
        match self {
            Data::PublicSentried(data) => data.expected_owners_index(),
            Data::Public(data) => data.expected_owners_index(),
            Data::PrivateSentried(data) => data.expected_owners_index(),
            Data::Private(data) => data.expected_owners_index(),
        }
    }

    pub fn in_range(&self, start: Index, end: Index) -> Option<Values> {
        match self {
            Data::PublicSentried(data) => data.in_range(start, end),
            Data::Public(data) => data.in_range(start, end),
            Data::PrivateSentried(data) => data.in_range(start, end),
            Data::Private(data) => data.in_range(start, end),
        }
    }

    pub fn get(&self, index: u64) -> Option<&Value> {
        match self {
            Data::PublicSentried(data) => data.get(index),
            Data::Public(data) => data.get(index),
            Data::PrivateSentried(data) => data.get(index),
            Data::Private(data) => data.get(index),
        }
    }

    pub fn indices(&self) -> ExpectedIndices {
        match self {
            Data::PublicSentried(data) => data.indices(),
            Data::Public(data) => data.indices(),
            Data::PrivateSentried(data) => data.indices(),
            Data::Private(data) => data.indices(),
        }
    }

    pub fn current_data_entry(&self) -> Option<DataEntry> {
        match self {
            Data::PublicSentried(data) => data.current_data_entry(),
            Data::Public(data) => data.current_data_entry(),
            Data::PrivateSentried(data) => data.current_data_entry(),
            Data::Private(data) => data.current_data_entry(),
        }
    }

    pub fn owner_at(&self, index: impl Into<Index>) -> Option<&Owner> {
        match self {
            Data::PublicSentried(data) => data.owner_at(index),
            Data::Public(data) => data.owner_at(index),
            Data::PrivateSentried(data) => data.owner_at(index),
            Data::Private(data) => data.owner_at(index),
        }
    }

    pub fn is_owner(&self, user: PublicKey) -> bool {
        match self {
            Data::PublicSentried(data) => data.is_owner(user),
            Data::Public(data) => data.is_owner(user),
            Data::PrivateSentried(data) => data.is_owner(user),
            Data::Private(data) => data.is_owner(user),
        }
    }

    pub fn public_user_permissions_at(
        &self,
        user: User,
        index: impl Into<Index>,
    ) -> Result<PublicPermissionSet> {
        self.public_permissions_at(index)?
            .permissions()
            .get(&user)
            .cloned()
            .ok_or(Error::NoSuchEntry)
    }

    pub fn private_user_permissions_at(
        &self,
        user: PublicKey,
        index: impl Into<Index>,
    ) -> Result<PrivatePermissionSet> {
        self.private_permissions_at(index)?
            .permissions()
            .get(&user)
            .cloned()
            .ok_or(Error::NoSuchEntry)
    }

    pub fn public_permissions_at(&self, index: impl Into<Index>) -> Result<&PublicPermissions> {
        let permissions = match self {
            Data::PublicSentried(data) => data.permissions_at(index),
            Data::Public(data) => data.permissions_at(index),
            _ => return Err(Error::NoSuchData),
        };
        permissions.ok_or(Error::NoSuchEntry)
    }

    pub fn private_permissions_at(&self, index: impl Into<Index>) -> Result<&PrivatePermissions> {
        let permissions = match self {
            Data::PrivateSentried(data) => data.permissions_at(index),
            Data::Private(data) => data.permissions_at(index),
            _ => return Err(Error::NoSuchData),
        };
        permissions.ok_or(Error::NoSuchEntry)
    }

    pub fn shell(&self, index: impl Into<Index>) -> Result<Self> {
        match self {
            Data::PublicSentried(adata) => adata.shell(index).map(Data::PublicSentried),
            Data::Public(adata) => adata.shell(index).map(Data::Public),
            Data::PrivateSentried(adata) => adata.shell(index).map(Data::PrivateSentried),
            Data::Private(adata) => adata.shell(index).map(Data::Private),
        }
    }
}

impl From<PublicSentriedSequence> for Data {
    fn from(data: PublicSentriedSequence) -> Self {
        Data::PublicSentried(data)
    }
}

impl From<PublicSequence> for Data {
    fn from(data: PublicSequence) -> Self {
        Data::Public(data)
    }
}

impl From<PrivateSentriedSequence> for Data {
    fn from(data: PrivateSentriedSequence) -> Self {
        Data::PrivateSentried(data)
    }
}

impl From<PrivateSequence> for Data {
    fn from(data: PrivateSequence) -> Self {
        Data::Private(data)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct AppendOperation {
    // Address of an Sequence object on the network.
    pub address: Address,
    // A list of Values to append.
    pub values: Values,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use threshold_crypto::SecretKey;
    //use unwrap::{unwrap, unwrap_err};
    use crate::permissions::{
        CmdType, HardErasureCmd, ModifyableSequencePermissions, QueryType, SequenceCmd,
        SequenceQuery, SequenceWrite,
    };
    use unwrap::unwrap;

    pub fn get_append_cmd() -> Request {
        Request::Cmd(CmdType::Sequence(SequenceCmd::Append))
    }

    fn get_read_query(query: SequenceQuery) -> Request {
        Request::Query(QueryType::Sequence(query))
    }

    fn get_full_read_permissions() -> Vec<Request> {
        vec![
            Request::Query(QueryType::Sequence(SequenceQuery::ReadData)),
            Request::Query(QueryType::Sequence(SequenceQuery::ReadOwners)),
            Request::Query(QueryType::Sequence(SequenceQuery::ReadPermissions)),
        ]
    }

    fn get_modify_permissions(permission: ModifyableSequencePermissions) -> Request {
        Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
            permission,
        )))
    }

    fn get_full_modify_permissions() -> Vec<Request> {
        vec![
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::ReadData,
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::ReadOwners,
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::ReadPermissions,
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::Write(SequenceWrite::Append),
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::Write(SequenceWrite::ModifyPermissions),
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::Write(SequenceWrite::HardErasure(
                    HardErasureCmd::HardDelete,
                )),
            ))),
            Request::Cmd(CmdType::Sequence(SequenceCmd::ModifyPermissions(
                ModifyableSequencePermissions::Write(SequenceWrite::HardErasure(
                    HardErasureCmd::HardUpdate,
                )),
            ))),
        ]
    }

    pub fn assert_read_permitted(data: &Data, public_key: PublicKey, permitted: bool) {
        assert_eq!(
            data.is_permitted(get_read_query(SequenceQuery::ReadData), public_key),
            permitted
        );
        assert_eq!(
            data.is_permitted(get_read_query(SequenceQuery::ReadOwners), public_key),
            permitted
        );
        assert_eq!(
            data.is_permitted(get_read_query(SequenceQuery::ReadPermissions), public_key),
            permitted
        );
    }

    pub fn assert_modify_permissions_permitted(
        data: &Data,
        public_key: PublicKey,
        permitted: bool,
    ) {
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::ReadData),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::ReadOwners),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::ReadPermissions),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::Write(SequenceWrite::Append)),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::Write(
                    SequenceWrite::ModifyPermissions
                )),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::Write(
                    SequenceWrite::HardErasure(HardErasureCmd::HardDelete)
                )),
                public_key
            ),
            permitted
        );
        assert_eq!(
            data.is_permitted(
                get_modify_permissions(ModifyableSequencePermissions::Write(
                    SequenceWrite::HardErasure(HardErasureCmd::HardUpdate)
                )),
                public_key
            ),
            permitted
        );
    }

    #[test]
    fn set_permissions() {
        let mut data = PrivateSentriedSequence::new(XorName([1; 32]), 10000);

        // Set the first permissions with correct ExpectedIndices - should pass.
        let res = data.set_permissions(
            PrivatePermissions {
                permissions: BTreeMap::new(),
                expected_data_index: 0,
                expected_owners_index: 0,
            },
            0,
        );

        match res {
            Ok(()) => (),
            Err(x) => panic!("Unexpected error: {:?}", x),
        }

        // Verify that the permissions are part of the history.
        assert_eq!(
            unwrap!(data.permission_history_range(Index::FromStart(0), Index::FromEnd(0),)).len(),
            1
        );

        // Set permissions with incorrect ExpectedIndices - should fail.
        let res = data.set_permissions(
            PrivatePermissions {
                permissions: BTreeMap::new(),
                expected_data_index: 64,
                expected_owners_index: 0,
            },
            1,
        );

        match res {
            Err(_) => (),
            Ok(()) => panic!("Unexpected Ok(()) result"),
        }

        // Verify that the history of permissions remains unchanged.
        assert_eq!(
            unwrap!(data.permission_history_range(Index::FromStart(0), Index::FromEnd(0),)).len(),
            1
        );
    }

    #[test]
    fn set_owners() {
        let owner_pk = gen_public_key();

        let mut data = PrivateSentriedSequence::new(XorName([1; 32]), 10000);

        // Set the first owner with correct ExpectedIndices - should pass.
        let res = data.set_owner(
            Owner {
                public_key: owner_pk,
                expected_data_index: 0,
                expected_permissions_index: 0,
            },
            0,
        );

        match res {
            Ok(()) => (),
            Err(x) => panic!("Unexpected error: {:?}", x),
        }

        // Verify that the owner is part of the history.
        assert_eq!(
            unwrap!(data.owner_history_range(Index::FromStart(0), Index::FromEnd(0),)).len(),
            1
        );

        // Set owner with incorrect ExpectedIndices - should fail.
        let res = data.set_owner(
            Owner {
                public_key: owner_pk,
                expected_data_index: 64,
                expected_permissions_index: 0,
            },
            1,
        );

        match res {
            Err(_) => (),
            Ok(()) => panic!("Unexpected Ok(()) result"),
        }

        // Verify that the history of owners remains unchanged.
        assert_eq!(
            unwrap!(data.owner_history_range(Index::FromStart(0), Index::FromEnd(0),)).len(),
            1
        );
    }

    #[test]
    fn append_sentried_data() {
        let mut data = PublicSentriedSequence::new(XorName([1; 32]), 10000);
        unwrap!(data.append(vec![b"hello".to_vec(), b"world".to_vec()], 0));
    }

    #[test]
    fn assert_shell() {
        let owner_pk = gen_public_key();
        let owner_pk1 = gen_public_key();

        let mut data = PrivateSentriedSequence::new(XorName([1; 32]), 10000);

        let _ = data.set_owner(
            Owner {
                public_key: owner_pk,
                expected_data_index: 0,
                expected_permissions_index: 0,
            },
            0,
        );

        let _ = data.set_owner(
            Owner {
                public_key: owner_pk1,
                expected_data_index: 0,
                expected_permissions_index: 0,
            },
            1,
        );

        assert_eq!(
            data.expected_owners_index(),
            unwrap!(data.shell(0)).expected_owners_index()
        );
    }

    #[test]
    fn zbase32_encode_decode_adata_address() {
        let name = XorName(rand::random());
        let address = Address::PrivateSentried { name, tag: 15000 };
        let encoded = address.encode_to_zbase32();
        let decoded = unwrap!(self::Address::decode_from_zbase32(&encoded));
        assert_eq!(address, decoded);
    }

    #[test]
    fn append_private_data() {
        let mut data = PrivateSequence::new(XorName(rand::random()), 10);

        // Assert that the Values are appended.
        let values1 = vec![
            b"KEY1".to_vec(),
            b"VALUE1".to_vec(),
            b"KEY2".to_vec(),
            b"VALUE2".to_vec(),
        ];

        unwrap!(data.append(values1));
    }

    #[test]
    fn append_private_sentried_data() {
        let mut data = PrivateSentriedSequence::new(XorName(rand::random()), 10);

        // Assert that the values are appended.
        let values1 = vec![
            b"KEY1".to_vec(),
            b"VALUE1".to_vec(),
            b"KEY2".to_vec(),
            b"VALUE2".to_vec(),
        ];
        unwrap!(data.append(values1, 0));
    }

    #[test]
    fn in_range() {
        let mut data = PublicSentriedSequence::new(rand::random(), 10);
        let values = vec![
            b"key0".to_vec(),
            b"value0".to_vec(),
            b"key1".to_vec(),
            b"value1".to_vec(),
        ];
        unwrap!(data.append(values, 0));

        assert_eq!(
            data.in_range(Index::FromStart(0), Index::FromStart(0)),
            Some(vec![])
        );
        assert_eq!(
            data.in_range(Index::FromStart(0), Index::FromStart(2)),
            Some(vec![b"key0".to_vec(), b"value0".to_vec()])
        );
        assert_eq!(
            data.in_range(Index::FromStart(0), Index::FromStart(4)),
            Some(vec![
                b"key0".to_vec(),
                b"value0".to_vec(),
                b"key1".to_vec(),
                b"value1".to_vec(),
            ])
        );

        assert_eq!(
            data.in_range(Index::FromEnd(4), Index::FromEnd(2)),
            Some(vec![b"key0".to_vec(), b"value0".to_vec(),])
        );
        assert_eq!(
            data.in_range(Index::FromEnd(4), Index::FromEnd(0)),
            Some(vec![
                b"key0".to_vec(),
                b"value0".to_vec(),
                b"key1".to_vec(),
                b"value1".to_vec(),
            ])
        );

        assert_eq!(
            data.in_range(Index::FromStart(0), Index::FromEnd(0)),
            Some(vec![
                b"key0".to_vec(),
                b"value0".to_vec(),
                b"key1".to_vec(),
                b"value1".to_vec(),
            ])
        );

        // start > end
        assert_eq!(
            data.in_range(Index::FromStart(1), Index::FromStart(0)),
            None
        );
        assert_eq!(data.in_range(Index::FromEnd(1), Index::FromEnd(2)), None);

        // overflow
        assert_eq!(
            data.in_range(Index::FromStart(0), Index::FromStart(5)),
            None
        );
        assert_eq!(data.in_range(Index::FromEnd(5), Index::FromEnd(0)), None);
    }

    #[test]
    fn can_retrieve_permissions() {
        let public_key = gen_public_key();
        let invalid_public_key = gen_public_key();

        let mut pub_permissions = PublicPermissions {
            permissions: BTreeMap::new(),
            expected_data_index: 0,
            expected_owners_index: 0,
        };
        let _ = pub_permissions.permissions.insert(
            User::Specific(public_key),
            PublicPermissionSet::new(BTreeMap::new()),
        );

        let mut private_permissions = PrivatePermissions {
            permissions: BTreeMap::new(),
            expected_data_index: 0,
            expected_owners_index: 0,
        };
        let _ = private_permissions
            .permissions
            .insert(public_key, PrivatePermissionSet::new(BTreeMap::new()));

        // pub, unseq
        let mut data = PublicSequence::new(rand::random(), 20);
        unwrap!(data.set_permissions(pub_permissions.clone(), 0));
        let data = Data::from(data);

        assert_eq!(data.public_permissions_at(0), Ok(&pub_permissions));
        assert_eq!(data.private_permissions_at(0), Err(Error::NoSuchData));

        assert_eq!(
            data.public_user_permissions_at(User::Specific(public_key), 0),
            Ok(PublicPermissionSet::new(BTreeMap::new()))
        );
        assert_eq!(
            data.private_user_permissions_at(public_key, 0),
            Err(Error::NoSuchData)
        );
        assert_eq!(
            data.public_user_permissions_at(User::Specific(invalid_public_key), 0),
            Err(Error::NoSuchEntry)
        );

        // pub, seq
        let mut data = PublicSentriedSequence::new(rand::random(), 20);
        unwrap!(data.set_permissions(pub_permissions.clone(), 0));
        let data = Data::from(data);

        assert_eq!(data.public_permissions_at(0), Ok(&pub_permissions));
        assert_eq!(data.private_permissions_at(0), Err(Error::NoSuchData));

        assert_eq!(
            data.public_user_permissions_at(User::Specific(public_key), 0),
            Ok(PublicPermissionSet::new(BTreeMap::new()))
        );
        assert_eq!(
            data.private_user_permissions_at(public_key, 0),
            Err(Error::NoSuchData)
        );
        assert_eq!(
            data.public_user_permissions_at(User::Specific(invalid_public_key), 0),
            Err(Error::NoSuchEntry)
        );

        // Private, unseq
        let mut data = PrivateSequence::new(rand::random(), 20);
        unwrap!(data.set_permissions(private_permissions.clone(), 0));
        let data = Data::from(data);

        assert_eq!(data.private_permissions_at(0), Ok(&private_permissions));
        assert_eq!(data.public_permissions_at(0), Err(Error::NoSuchData));

        assert_eq!(
            data.private_user_permissions_at(public_key, 0),
            Ok(PrivatePermissionSet::new(BTreeMap::new()))
        );
        assert_eq!(
            data.public_user_permissions_at(User::Specific(public_key), 0),
            Err(Error::NoSuchData)
        );
        assert_eq!(
            data.private_user_permissions_at(invalid_public_key, 0),
            Err(Error::NoSuchEntry)
        );

        // Private, seq
        let mut data = PrivateSentriedSequence::new(rand::random(), 20);
        unwrap!(data.set_permissions(private_permissions.clone(), 0));
        let data = Data::from(data);

        assert_eq!(data.private_permissions_at(0), Ok(&private_permissions));
        assert_eq!(data.public_permissions_at(0), Err(Error::NoSuchData));

        assert_eq!(
            data.private_user_permissions_at(public_key, 0),
            Ok(PrivatePermissionSet::new(BTreeMap::new()))
        );
        assert_eq!(
            data.public_user_permissions_at(User::Specific(public_key), 0),
            Err(Error::NoSuchData)
        );
        assert_eq!(
            data.private_user_permissions_at(invalid_public_key, 0),
            Err(Error::NoSuchEntry)
        );
    }

    fn gen_public_key() -> PublicKey {
        PublicKey::Bls(SecretKey::random().public_key())
    }

    #[test]
    fn validates_public_permissions() {
        let public_key_0 = gen_public_key();
        let public_key_1 = gen_public_key();
        let public_key_2 = gen_public_key();
        let mut map = PublicSentriedSequence::new(XorName([1; 32]), 100);

        // no owner
        let data = Data::from(map.clone());
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_0), false);
        // data is Public - read always allowed
        assert_read_permitted(&data, public_key_0, true);

        // no permissions
        unwrap!(map.set_owner(
            Owner {
                public_key: public_key_0,
                expected_data_index: 0,
                expected_permissions_index: 0,
            },
            0,
        ));
        let data = Data::from(map.clone());

        assert_eq!(data.is_permitted(get_append_cmd(), public_key_0), true);
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_1), false);
        // data is Public - read always allowed
        assert_read_permitted(&data, public_key_0, true);
        assert_read_permitted(&data, public_key_1, true);

        // with permissions
        let mut permissions = PublicPermissions {
            permissions: BTreeMap::new(),
            expected_data_index: 0,
            expected_owners_index: 1,
        };
        let mut set = BTreeMap::new();
        let _ = set.insert(get_append_cmd(), true);
        let _ = permissions
            .permissions
            .insert(User::Anyone, PublicPermissionSet::new(set));
        let mut set = BTreeMap::new();
        for cmd in get_full_modify_permissions() {
            let _ = set.insert(cmd, true);
        }
        let _ = permissions
            .permissions
            .insert(User::Specific(public_key_1), PublicPermissionSet::new(set));
        unwrap!(map.set_permissions(permissions, 0));
        let data = Data::from(map);

        // existing key fallback
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_1), true);
        // existing key override
        assert_modify_permissions_permitted(&data, public_key_1, true);
        // non-existing keys are handled by `Anyone`
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_2), true);
        assert_modify_permissions_permitted(&data, public_key_2, false);
        // data is Public - read always allowed
        assert_read_permitted(&data, public_key_0, true);
        assert_read_permitted(&data, public_key_1, true);
        assert_read_permitted(&data, public_key_2, true);
    }

    #[test]
    fn validates_private_permissions() {
        let public_key_0 = gen_public_key();
        let public_key_1 = gen_public_key();
        let public_key_2 = gen_public_key();
        let mut map = PrivateSentriedSequence::new(XorName([1; 32]), 100);

        // no owner
        let data = Data::from(map.clone());
        assert_read_permitted(&data, public_key_0, false);

        // no permissions
        unwrap!(map.set_owner(
            Owner {
                public_key: public_key_0,
                expected_data_index: 0,
                expected_permissions_index: 0,
            },
            0,
        ));
        let data = Data::from(map.clone());

        assert_read_permitted(&data, public_key_0, true);
        assert_read_permitted(&data, public_key_1, false);

        // with permissions
        let mut permissions = PrivatePermissions {
            permissions: BTreeMap::new(),
            expected_data_index: 0,
            expected_owners_index: 1,
        };
        let mut set = BTreeMap::new();
        let _ = set.insert(get_append_cmd(), true);
        for query in get_full_read_permissions() {
            let _ = set.insert(query, true);
        }
        for cmd in get_full_modify_permissions() {
            let _ = set.insert(cmd, false);
        }
        let _ = permissions
            .permissions
            .insert(public_key_1, PrivatePermissionSet::new(set));
        unwrap!(map.set_permissions(permissions, 0));
        let data = Data::from(map);

        // existing key
        assert_read_permitted(&data, public_key_1, true);
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_1), true);
        assert_modify_permissions_permitted(&data, public_key_1, false);

        // non-existing key
        assert_read_permitted(&data, public_key_2, false);
        assert_eq!(data.is_permitted(get_append_cmd(), public_key_2), false);
        assert_modify_permissions_permitted(&data, public_key_2, false);
    }
}
