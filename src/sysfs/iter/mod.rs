use std::{fs::ReadDir, io, path::Path};

use crate::Devno;

use super::SysFs;

struct DirIterator {
    dir: ReadDir,
}

impl DirIterator {
    #[inline]
    pub(crate) fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self {
            dir: std::fs::read_dir(p)?,
        })
    }
}

impl Iterator for DirIterator {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        let dev = match self.dir.next()? {
            Ok(dev) => dev,
            Err(err) => return Some(Err(err)),
        }
        .path();

        match match std::fs::read_to_string(dev.join("dev")) {
            Ok(d) => d,
            Err(err) => return Some(Err(err)),
        }
        .trim()
        .parse::<Devno>()
        {
            Ok(devno) => Some(Ok(devno)),
            Err(_) => Some(Err(io::ErrorKind::InvalidData.into())),
        }
    }
}

pub struct DisksIterator<'a> {
    sysfs: &'a SysFs,
    inner: DirIterator,
}

impl<'a> DisksIterator<'a> {
    #[inline]
    pub(crate) fn new(sysfs: &'a SysFs) -> io::Result<Self> {
        Ok(Self {
            inner: DirIterator::new(sysfs.path().join("block"))?,
            sysfs,
        })
    }
}

impl<'a> Iterator for DisksIterator<'a> {
    type Item = io::Result<Devno>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Ok(devno) => match self.sysfs.is_wholedisk(&devno) {
                    Ok(true) => return Some(Ok(devno)),
                    Ok(false) => (),
                    Err(err) => return Some(Err(err)),
                },
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

pub struct BlocksIterator {
    inner: DirIterator,
}

impl BlocksIterator {
    #[inline]
    pub(crate) fn new(sysfs: &SysFs) -> io::Result<Self> {
        Ok(Self {
            inner: DirIterator::new(sysfs.path().join("dev").join("block"))?,
        })
    }
}

impl Iterator for BlocksIterator {
    type Item = io::Result<Devno>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
