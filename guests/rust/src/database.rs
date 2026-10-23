use crate::{msgpack, raw, Error, IteratorType, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{marker::PhantomData, rc::Rc};

/// Resolved catalog identity; resolve names again after changing the schema.
#[derive(Clone, Copy, Debug)]
pub struct Space {
    id: u32,
}

impl Space {
    pub fn new(id: u32) -> Self {
        Self { id }
    }

    pub fn by_name(name: &str) -> Result<Option<Self>> {
        Ok(raw::database::space_by_name(name)?.map(|s| Self::new(s.id)))
    }

    pub fn id(self) -> u32 {
        self.id
    }

    fn raw(self) -> raw::types::Space {
        raw::types::Space { id: self.id }
    }

    pub fn primary(self) -> Index {
        self.index(0)
    }

    pub fn index(self, id: u32) -> Index {
        Index { space: self, id }
    }

    pub fn index_by_name(self, name: &str) -> Result<Option<Index>> {
        Ok(raw::database::index_by_name(self.raw(), name)?.map(|i| self.index(i.id)))
    }

    pub fn insert(self, tuple: &impl Serialize) -> Result<Tuple> {
        Ok(Tuple::owned(raw::database::insert(
            self.raw(),
            &msgpack::pack_array(tuple)?,
        )?))
    }

    pub fn replace(self, tuple: &impl Serialize) -> Result<Tuple> {
        Ok(Tuple::owned(raw::database::replace(
            self.raw(),
            &msgpack::pack_array(tuple)?,
        )?))
    }

    /// Update operations use the host's zero-based database field positions.
    pub fn upsert(self, tuple: &impl Serialize, operations: &impl Serialize) -> Result<()> {
        Ok(raw::database::upsert(
            self.primary().raw(),
            &msgpack::pack_array(tuple)?,
            &msgpack::pack_array(operations)?,
        )?)
    }

    pub fn truncate(self) -> Result<()> {
        Ok(raw::database::truncate(self.raw())?)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Index {
    space: Space,
    id: u32,
}

impl Index {
    pub fn id(self) -> u32 {
        self.id
    }

    fn raw(self) -> raw::types::Index {
        raw::types::Index {
            id: self.id,
            space_id: self.space.id,
        }
    }

    pub fn get(self, key: &impl Serialize) -> Result<Option<Tuple>> {
        Ok(raw::index::get(self.raw(), &msgpack::pack_array(key)?)?.map(Tuple::owned))
    }

    pub fn delete(self, key: &impl Serialize) -> Result<Option<Tuple>> {
        Ok(raw::database::delete(self.raw(), &msgpack::pack_array(key)?)?.map(Tuple::owned))
    }

    pub fn update(
        self,
        key: &impl Serialize,
        operations: &impl Serialize,
    ) -> Result<Option<Tuple>> {
        Ok(raw::database::update(
            self.raw(),
            &msgpack::pack_array(key)?,
            &msgpack::pack_array(operations)?,
        )?
        .map(Tuple::owned))
    }

    pub fn len(self) -> Result<u64> {
        Ok(raw::index::tuple_count(self.raw())?)
    }

    pub fn is_empty(self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    pub fn count(self, key: &impl Serialize, mode: IteratorType) -> Result<u64> {
        Ok(raw::index::count(
            self.raw(),
            mode,
            &msgpack::pack_array(key)?,
        )?)
    }

    /// Streaming scan. Drop releases the iterator, including on early exit.
    pub fn scan(self, key: &impl Serialize, mode: IteratorType) -> Result<Scan> {
        Ok(Scan {
            inner: Some(raw::index::Iterator::new(
                self.raw(),
                mode,
                &msgpack::pack_array(key)?,
            )?),
            marker: PhantomData,
        })
    }
}

/// Owns exactly one host tuple reference. No raw handle constructor is exposed.
#[derive(Debug)]
pub struct Tuple {
    raw: raw::types::BoxTuple,
    marker: PhantomData<Rc<()>>,
}

impl Tuple {
    fn owned(raw: raw::types::BoxTuple) -> Self {
        Self {
            raw,
            marker: PhantomData,
        }
    }

    pub fn new(value: &impl Serialize) -> Result<Self> {
        Ok(Self::owned(raw::box_tuple::new(&msgpack::pack_array(
            value,
        )?)?))
    }

    pub fn len(&self) -> u32 {
        raw::box_tuple::field_count(self.raw)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn to_vec(&self) -> Result<Vec<u8>> {
        Ok(raw::box_tuple::to_bytes(self.raw)?)
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        msgpack::unpack(&self.to_vec()?)
    }

    /// Outer Option distinguishes a missing field from a MessagePack nil.
    pub fn field<T: DeserializeOwned>(&self, index: u32) -> Result<Option<T>> {
        raw::box_tuple::field(self.raw, index)?
            .map(|b| msgpack::unpack(&b))
            .transpose()
    }
}

impl Clone for Tuple {
    fn clone(&self) -> Self {
        raw::box_tuple::retain(self.raw);
        Self::owned(self.raw)
    }
}

impl Drop for Tuple {
    fn drop(&mut self) {
        raw::box_tuple::release(self.raw);
    }
}

pub struct Scan {
    inner: Option<raw::index::Iterator>,
    marker: PhantomData<Rc<()>>,
}

impl Iterator for Scan {
    type Item = Result<Tuple>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.as_ref()?.next() {
            Ok(Some(tuple)) => Some(Ok(Tuple::owned(tuple))),
            Ok(None) => {
                self.inner.take();
                None
            }
            Err(error) => {
                self.inner.take();
                Some(Err(Error::Host(error)))
            }
        }
    }
}

impl std::iter::FusedIterator for Scan {}
