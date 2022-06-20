use std::{
    cell::{Ref, RefCell},
    collections::BTreeMap,
    fs::{DirEntry, ReadDir},
    io,
    os::unix::prelude::*,
    path::{Path, PathBuf},
};

use crate::{procfs::ProcFs, Devno};

pub struct BlocksIterator {
    dir: ReadDir,
    inner: Option<Box<BlocksIterator>>,
}

impl BlocksIterator {
    fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self {
            dir: std::fs::read_dir(p)?,
            inner: None,
        })
    }
}

impl Iterator for BlocksIterator {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut inner) = self.inner {
                if let Some(res) = inner.next() {
                    return Some(res);
                } else {
                    self.inner = None;
                }
            }

            let res = match self.dir.next()? {
                Ok(res) => res,
                Err(err) => return Some(Err(err)),
            };

            if res.path().is_symlink() {
                continue;
            }

            if res.file_name().as_bytes()[0] == b'.' {
                continue;
            }

            if res.file_name() == "shm" {
                continue;
            }

            if res.path().is_dir() {
                let inner = match std::fs::read_dir(res.path()) {
                    Ok(i) => i,
                    Err(err) => return Some(Err(err)),
                };

                self.inner = Some(Box::new(Self {
                    dir: inner,
                    inner: None,
                }));
                continue;
            }

            let md = match res.path().metadata() {
                Ok(md) => md,
                Err(err) => return Some(Err(err)),
            };

            if md.file_type().is_block_device() {
                return Some(Ok(res));
            }
        }
    }
}

pub struct DevFs {
    path: PathBuf,
    cache: RefCell<BTreeMap<Devno, PathBuf>>,
}

impl DevFs {
    pub fn new(procfs: &ProcFs) -> io::Result<Self> {
        let path = match procfs.mounts().find(|m| {
            m.file_system == "devtmpfs"
                && matches!(m.source.as_ref().map(|x| x.as_str()), Some("dev"))
        })? {
            Some(pp) => pp.mount_point,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "devfs not found")),
        };

        Ok(Self {
            path,
            cache: RefCell::new(BTreeMap::new()),
        })
    }

    #[inline]
    fn cache_get<'a>(&'a self, devno: &Devno) -> Option<Ref<'a, Path>> {
        let c = self.cache.borrow();
        c.get(devno)
            .map(|v| v.as_path())
            .map(|v| v as *const _)
            .map(|v| Ref::map(c, |_| unsafe { &*v }))
    }

    fn find_in_cache<'a>(&'a self, devno: &Devno) -> io::Result<Option<Ref<Path>>> {
        if let Some(p) = self.cache_get(devno) {
            if p.exists() {
                let md = p.metadata()?;
                if md.file_type().is_block_device() && Devno::from(md.rdev()) == *devno {
                    return Ok(Some(p));
                }
            }
        }

        self.cache.borrow_mut().remove(devno);
        Ok(None)
    }

    fn by_dev<'a>(&'a self, devno: &Devno) -> io::Result<Option<Ref<'a, Path>>> {
        if let Some(x) = self.find_in_cache(devno)? {
            return Ok(Some(x));
        }

        {
            for p in self.iter()? {
                let p = p?.path();

                if p.exists() {
                    let md = p.metadata()?;
                    let d = Devno::from(md.rdev());
                    {
                        self.cache.borrow_mut().insert(d, p);
                    }
                    let p = self.cache_get(&d).unwrap();

                    if d == *devno {
                        return Ok(Some(p));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn resolve(&self, devno: &Devno) -> io::Result<PathBuf> {
        let path = self.path.join("blocks").join(devno.to_string());

        if path.exists() {
            let path = path.canonicalize()?;
            let md = path.metadata()?;
            if md.file_type().is_block_device() {
                Ok(path)
            } else {
                Err(io::ErrorKind::InvalidInput.into())
            }
        } else {
            match self.by_dev(devno)? {
                Some(p) => Ok(p.to_path_buf()),
                None => Err(io::ErrorKind::NotFound.into()),
            }
        }
    }

    #[inline]
    pub fn iter(&self) -> io::Result<BlocksIterator> {
        BlocksIterator::new(&self.path)
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
