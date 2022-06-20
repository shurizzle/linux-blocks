use std::{
    cell::{Ref, RefCell},
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, BufRead, BufReader},
    ops::Deref,
    path::{Path, PathBuf},
};

use indexmap::IndexSet;

struct DevicesCache {
    names: IndexSet<String>,
    by_id: BTreeMap<u32, usize>,
    by_name: BTreeMap<usize, BTreeSet<u32>>,
}

impl DevicesCache {
    #[inline]
    pub fn new() -> Self {
        Self {
            names: IndexSet::new(),
            by_id: BTreeMap::new(),
            by_name: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, id: u32, name: String) {
        let idx = self.names.insert_full(name).0;
        self.by_id.insert(id, idx);
    }

    pub fn get_by_id(&self, id: u32) -> Option<&str> {
        self.by_id
            .get(&id)
            .and_then(|idx| self.names.get_index(*idx).map(|x| x.as_str()))
    }

    pub fn get_by_name<S: AsRef<str>>(&self, name: S) -> Option<&BTreeSet<u32>> {
        self.names
            .get_index_of(name.as_ref())
            .and_then(|idx| self.by_name.get(&idx))
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> DevicesCacheIter<'a> {
        DevicesCacheIter::new(&self)
    }
}

impl FromIterator<(u32, String)> for DevicesCache {
    fn from_iter<T: IntoIterator<Item = (u32, String)>>(iter: T) -> Self {
        let mut acc = DevicesCache::new();
        for (id, name) in iter {
            acc.insert(id, name);
        }
        acc
    }
}

impl<'a> FromIterator<(u32, &'a String)> for DevicesCache {
    fn from_iter<T: IntoIterator<Item = (u32, &'a String)>>(iter: T) -> Self {
        let mut acc = DevicesCache::new();
        for (id, name) in iter {
            acc.insert(id, name.to_string());
        }
        acc
    }
}

impl<'a> FromIterator<(u32, &'a str)> for DevicesCache {
    fn from_iter<T: IntoIterator<Item = (u32, &'a str)>>(iter: T) -> Self {
        let mut acc = DevicesCache::new();
        for (id, name) in iter {
            acc.insert(id, name.to_string());
        }
        acc
    }
}

struct DevicesCacheIter<'a> {
    names: &'a IndexSet<String>,
    inner: std::collections::btree_map::Iter<'a, u32, usize>,
}

impl<'a> DevicesCacheIter<'a> {
    #[inline]
    pub fn new(cache: &'a DevicesCache) -> Self {
        Self {
            names: &cache.names,
            inner: cache.by_id.iter(),
        }
    }
}

impl<'a> Iterator for DevicesCacheIter<'a> {
    type Item = (u32, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (&id, &idx) = self.inner.next()?;
        self.names.get_index(idx).map(|n| (id, n.as_str()))
    }
}

fn skip_proc_devices_header(x: &io::Result<String>) -> bool {
    match x {
        Ok(s) => !s.starts_with("Block devices:"),
        Err(_) => false,
    }
}

pub struct DeviceIterator {
    inner: Box<dyn Iterator<Item = io::Result<String>>>,
}

impl DeviceIterator {
    #[inline]
    pub fn new<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Self {
            inner: Box::new(
                BufReader::new(File::open(p.as_ref().join("devices"))?)
                    .lines()
                    .skip_while(skip_proc_devices_header)
                    .skip(1),
            ),
        })
    }

    fn remap(x: io::Result<String>) -> io::Result<(u32, String)> {
        match x {
            Ok(line) => {
                let mut it = line.trim().split_whitespace();
                let id: u32 = it
                    .next()
                    .map_or_else(
                        || Err(Into::<io::Error>::into(io::ErrorKind::InvalidInput)),
                        |x| Ok(x),
                    )
                    .and_then(|x| {
                        x.parse::<u32>()
                            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::InvalidInput))
                    })?;
                let name = it.next().map_or_else(
                    || Err(Into::<io::Error>::into(io::ErrorKind::InvalidInput)),
                    |x| Ok(x.trim()),
                )?;
                if !matches!(it.next(), None) {
                    Err(io::ErrorKind::InvalidInput.into())
                } else {
                    Ok((id, name.to_lowercase()))
                }
            }
            Err(err) => Err(err),
        }
    }
}

impl Iterator for DeviceIterator {
    type Item = io::Result<(u32, String)>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some(Self::remap(self.inner.next()?))
    }
}

pub struct Devices {
    proc_path: PathBuf,
    cache: RefCell<DevicesCache>,
}

impl Devices {
    #[inline]
    pub fn new<P: AsRef<Path>>(p: P) -> Self {
        Self {
            proc_path: p.as_ref().to_path_buf(),
            cache: RefCell::new(DevicesCache::new()),
        }
    }

    fn _get_by_id<'a>(&'a self, id: u32) -> Option<Ref<'a, str>> {
        let cache = self.cache.borrow();
        cache
            .get_by_id(id)
            .map(|s| s as *const _)
            .map(|ptr| Ref::map(cache, |_| unsafe { &*ptr }))
    }

    fn _get_by_name<'a, S: AsRef<str>>(&'a self, name: S) -> Option<Ref<'a, BTreeSet<u32>>> {
        let cache = self.cache.borrow();
        cache
            .get_by_name(name)
            .map(|s| s as *const _)
            .map(|p| Ref::map(cache, |_| unsafe { &*p }))
    }

    pub fn get_by_id<'a>(&'a self, id: u32) -> io::Result<Option<Ref<'a, str>>> {
        {
            if let Some(v) = self._get_by_id(id) {
                return Ok(Some(v));
            }
        }

        self.refresh()?;

        Ok(self._get_by_id(id))
    }

    pub fn get_by_name<'a, S: AsRef<str>>(
        &'a self,
        name: S,
    ) -> io::Result<Option<Ref<'a, BTreeSet<u32>>>> {
        {
            if let Some(v) = self._get_by_name(name.as_ref()) {
                return Ok(Some(v));
            }
        }

        self.refresh()?;

        Ok(self._get_by_name(name))
    }

    #[inline]
    pub fn iter(&self) -> DevicesIter {
        DevicesIter::new(self.cache.borrow())
    }

    pub fn refresh(&self) -> io::Result<()> {
        self.cache
            .replace(DeviceIterator::new(&self.proc_path)?.collect::<io::Result<DevicesCache>>()?);
        Ok(())
    }
}

pub struct DevicesIter<'a> {
    holder: Ref<'a, DevicesCache>,
    inner: DevicesCacheIter<'a>,
}

impl<'a> DevicesIter<'a> {
    #[inline]
    fn new(holder: Ref<'a, DevicesCache>) -> Self {
        let inner = unsafe { &*(Ref::deref(&holder) as *const DevicesCache) }.iter();
        Self { holder, inner }
    }
}

impl<'a> Iterator for DevicesIter<'a> {
    type Item = (u32, Ref<'a, str>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (id, name) = self.inner.next()?;
        let name = Ref::map(Ref::clone(&self.holder), |_| unsafe {
            &*(name as *const _)
        });
        Some((id, name))
    }
}
