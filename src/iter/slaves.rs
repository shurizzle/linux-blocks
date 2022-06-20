use std::{
    fs::ReadDir,
    io,
    iter::{empty, Empty},
    path::Path,
};

use crate::Devno;

pub struct RawSlavesIterator {
    dir: ReadDir,
}

impl RawSlavesIterator {
    #[inline]
    pub fn new<P: AsRef<Path>>(slaves_dir: P) -> io::Result<Self> {
        Ok(Self {
            dir: std::fs::read_dir(slaves_dir)?,
        })
    }
}

impl Iterator for RawSlavesIterator {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        let devdir = match self.dir.next()? {
            Ok(devdir) => devdir,
            Err(err) => return Some(Err(err)),
        };

        match std::fs::read_to_string(devdir.path().join("dev")) {
            Ok(content) => match content.trim().parse::<Devno>() {
                Ok(devno) => Some(Ok(devno)),
                Err(_) => Some(Err(io::ErrorKind::InvalidData.into())),
            },
            Err(err) => Some(Err(err)),
        }
    }
}

enum InnerSlavesIterator {
    Iter(RawSlavesIterator),
    Empty(Empty<io::Result<Devno>>),
}

impl InnerSlavesIterator {
    #[inline]
    pub fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self::Iter(RawSlavesIterator::new(p)?))
    }

    #[inline]
    pub fn empty() -> io::Result<Self> {
        Ok(Self::Empty(empty()))
    }
}

impl Iterator for InnerSlavesIterator {
    type Item = io::Result<Devno>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Iter(ref mut it) => it.next(),
            Self::Empty(ref mut it) => it.next(),
        }
    }
}

pub struct SlavesIterator(InnerSlavesIterator);

impl SlavesIterator {
    #[inline]
    pub(crate) fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        let p = p.as_ref().join("slaves");
        if p.exists() {
            InnerSlavesIterator::new(p).map(Self)
        } else {
            Self::empty()
        }
    }

    #[inline]
    pub(crate) fn empty() -> io::Result<Self> {
        InnerSlavesIterator::empty().map(Self)
    }
}

impl Iterator for SlavesIterator {
    type Item = io::Result<Devno>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
