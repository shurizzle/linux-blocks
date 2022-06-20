use std::{
    fs::ReadDir,
    io,
    iter::{empty, Empty},
    path::Path,
};

use crate::{blocks, sysfs::iter::BlocksIterator, Devno};

struct RawPartitionsIterator {
    dir: ReadDir,
}

impl RawPartitionsIterator {
    #[inline]
    pub fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self {
            dir: std::fs::read_dir(p)?,
        })
    }
}

impl Iterator for RawPartitionsIterator {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = match self.dir.next()? {
                Ok(e) => e,
                Err(err) => return Some(Err(err)),
            }
            .path();

            if !entry.is_dir() {
                continue;
            }

            if entry.join("partition").is_file() {
                match std::fs::read_to_string(entry.join("dev")) {
                    Ok(content) => match content.trim().parse::<Devno>() {
                        Ok(devno) => return Some(Ok(devno)),
                        Err(_) => return Some(Err(io::ErrorKind::InvalidData.into())),
                    },
                    Err(err) => return Some(Err(err)),
                }
            }
        }
    }
}

struct MastersIterator<'a> {
    blocks: &'a blocks::Blocks,
    slave: Devno,
    inner: BlocksIterator,
}

impl<'a> MastersIterator<'a> {
    pub(crate) fn new(blocks: &'a blocks::Blocks, slave: Devno) -> io::Result<Self> {
        Ok(Self {
            inner: blocks.blocks()?,
            blocks,
            slave,
        })
    }
}

impl<'a> Iterator for MastersIterator<'a> {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let devno = match self.inner.next()? {
                Ok(devno) => devno,
                Err(err) => return Some(Err(err)),
            };

            match self.blocks.parent(&devno) {
                Ok(Some(parent)) => {
                    if parent == self.slave {
                        return Some(Ok(devno));
                    }
                }
                Ok(None) => (),
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

enum InternalPartitionsIterator<'a> {
    Masters(MastersIterator<'a>),
    Partitions(RawPartitionsIterator),
    Empty(Empty<io::Result<Devno>>),
}

impl<'a> InternalPartitionsIterator<'a> {
    #[inline]
    pub fn masters(blocks: &'a blocks::Blocks, devno: Devno) -> io::Result<Self> {
        Ok(Self::Masters(MastersIterator::new(blocks, devno)?))
    }

    #[inline]
    pub fn partitions<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self::Partitions(RawPartitionsIterator::new(p)?))
    }

    #[inline]
    pub fn empty() -> io::Result<Self> {
        Ok(Self::Empty(empty()))
    }
}

impl<'a> Iterator for InternalPartitionsIterator<'a> {
    type Item = io::Result<Devno>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Masters(ref mut it) => it.next(),
            Self::Partitions(ref mut it) => it.next(),
            Self::Empty(ref mut it) => it.next(),
        }
    }
}

pub struct PartitionsIterator<'a>(InternalPartitionsIterator<'a>);

impl<'a> PartitionsIterator<'a> {
    #[inline]
    pub(crate) fn masters(blocks: &'a blocks::Blocks, devno: Devno) -> io::Result<Self> {
        InternalPartitionsIterator::masters(blocks, devno).map(Self)
    }

    #[inline]
    pub(crate) fn partitions<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        InternalPartitionsIterator::partitions(p).map(Self)
    }

    #[inline]
    pub(crate) fn empty() -> io::Result<Self> {
        InternalPartitionsIterator::empty().map(Self)
    }
}

impl<'a> Iterator for PartitionsIterator<'a> {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
