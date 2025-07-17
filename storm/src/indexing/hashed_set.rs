use nohash::{IntMap};
use roaring::RoaringBitmap;
use std::{
    cmp::{max, min}, fmt::{self, Debug, Formatter}, hash::{Hash, Hasher}, ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Deref}
};

type IntSet = nohash::IntSet<u32>;

#[derive(Clone)]
pub struct HashedSet {
    set: RoaringBitmap,
    xor: u32,
}

impl HashedSet {
    #[inline]
    pub fn contains(&self, value: u32) -> bool {
        self.set.contains(value)
    }

    pub fn insert(&mut self, value: u32) -> bool {
        let out = self.set.insert(value);

        if out {
            self.xor ^= value;
        }

        out
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.set.len() as usize
    }

    fn rehash(&mut self) {
        let mut xor = 0;

        for v in &self.set {
            xor ^= v;
        }

        self.xor = xor;
    }

    pub fn remove(&mut self, value: u32) -> bool {
        let out = self.set.remove(value);

        if out {
            self.xor ^= value;
        }

        out
    }
}

impl Debug for HashedSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.set, f)
    }
}

impl Deref for HashedSet {
    type Target = RoaringBitmap;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.set
    }
}

impl Eq for HashedSet {}

impl Hash for HashedSet {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.xor.hash(state);
        self.set.len().hash(state);
    }
}

impl PartialEq for HashedSet {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.xor == other.xor && self.set == other.set
    }
}

impl BitAnd for HashedSet {
    type Output = HashedSet;

    fn bitand(mut self, rhs: Self) -> Self::Output {
        self.set &= &rhs.set;
        self.rehash();
        self
    }
}

impl BitAnd for &HashedSet {
    type Output = HashedSet;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut set = HashedSet {
            set: &self.set & &rhs.set,
            xor: 0,
        };

        set.rehash();
        set
    }
}

impl BitAnd<HashedSet> for &HashedSet {
    type Output = HashedSet;

    fn bitand(self, mut rhs: HashedSet) -> Self::Output {
        rhs.set &= &self.set;
        rhs.rehash();
        rhs
    }
}

impl BitAnd<&HashedSet> for HashedSet {
    type Output = HashedSet;

    fn bitand(mut self, rhs: &Self) -> Self::Output {
        self.set &= &rhs.set;
        self.rehash();
        self
    }
}

impl BitAndAssign for HashedSet {
    fn bitand_assign(&mut self, rhs: Self) {
        self.set &= rhs.set;
        self.rehash();
    }
}

impl BitAndAssign<&HashedSet> for HashedSet {
    fn bitand_assign(&mut self, rhs: &Self) {
        self.set &= &rhs.set;
        self.rehash();
    }
}

impl BitOr for HashedSet {
    type Output = HashedSet;

    fn bitor(mut self, rhs: Self) -> Self::Output {
        self.set |= rhs.set;
        self.rehash();
        self
    }
}

impl BitOr for &HashedSet {
    type Output = HashedSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut set = HashedSet {
            set: &self.set | &rhs.set,
            xor: 0,
        };

        set.rehash();
        set
    }
}

impl BitOr<HashedSet> for &HashedSet {
    type Output = HashedSet;

    fn bitor(self, mut rhs: HashedSet) -> Self::Output {
        rhs.set |= &self.set;
        rhs.rehash();
        rhs
    }
}

impl BitOr<&HashedSet> for HashedSet {
    type Output = HashedSet;

    fn bitor(mut self, rhs: &HashedSet) -> Self::Output {
        self.set |= &rhs.set;
        self.rehash();
        self
    }
}

impl BitOrAssign for HashedSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.set |= rhs.set;
        self.rehash();
    }
}

impl BitOrAssign<&HashedSet> for HashedSet {
    fn bitor_assign(&mut self, rhs: &Self) {
        self.set |= &rhs.set;
        self.rehash();
    }
}


struct U32OneToMany {
    all: RoaringBitmap,
    set_min: u32,
    set_max: u32,
    map: IntMap<u32, IncExc>,
}

struct ExcludableSet {
    excluded: bool,
    set: IntSet<u32>,
}

impl ExcludableSet {
    fn contains(&self, value: u32) -> bool {
        self.set.contains(&value) ^ self.excluded
    }

    fn insert(&mut self, value: u32, min: u32, max: u32) -> (u32, u32, bool) {
        set.len() > (max - min) * 2
    }
}




struct Log {
    all: Option<RoaringBitmap>,
    map: IntMap<u32, ExcludableSet>,
}

impl Log {
    fn insert(&mut self, many: &U32OneToMany, key: u32, val: u32) -> bool {
        if !many.all.contains(val) {
            if self.all.is_none() {
                self.all = Some(many.all.clone());
            }

            unsafe { self.all.as_mut().unwrap_unchecked() }.insert(val);
        }

        todo!()
        
    }
}
