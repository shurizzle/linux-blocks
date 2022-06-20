mod partitions;
mod slaves;

use std::{borrow::Cow, io, rc::Rc};

pub use partitions::PartitionsIterator;
pub(crate) use slaves::RawSlavesIterator;
pub use slaves::SlavesIterator;

use crate::{blocks, Blocks, Device, Devno};

pub struct DevnoMapper<'a, I> {
    blocks: Cow<'a, Blocks>,
    inner: I,
}

impl<'a, I: Iterator<Item = io::Result<Devno>>> DevnoMapper<'a, I> {
    #[inline]
    pub fn new(blocks: &'a Blocks, inner: I) -> Self {
        Self {
            blocks: Cow::Borrowed(blocks),
            inner,
        }
    }

    pub(crate) fn from_raw(blocks: &Rc<blocks::Blocks>, inner: I) -> Self {
        Self {
            blocks: Cow::Owned(Blocks::from_inner(Rc::clone(blocks))),
            inner,
        }
    }
}

impl<'a, I: Iterator<Item = io::Result<Devno>>> Iterator for DevnoMapper<'a, I> {
    type Item = io::Result<Device>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok(devno) => Some(self.blocks.from_devno(devno)),
            Err(err) => Some(Err(err)),
        }
    }
}
