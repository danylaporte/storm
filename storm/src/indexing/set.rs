use roaring::{bitmap, RoaringBitmap};
use std::{
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign},
    sync::OnceLock,
};

use crate::Gc;

#[repr(transparent)]
pub struct Set<K>(pub(crate) RoaringBitmap, PhantomData<K>);

impl<K> Set<K> {
    pub fn new() -> Self {
        Self(RoaringBitmap::new(), PhantomData)
    }

    #[inline]
    pub(crate) fn from_roaring_bitmap_ref(b: &RoaringBitmap) -> &Self {
        // Safety: Because Set is #[repr(transparent)] with RoaringBitmap as its
        // only non-zero-sized field, their memory layouts are identical.
        // This allows us to safely transmute the reference.
        unsafe { &*(b as *const RoaringBitmap as *const Self) }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[inline]
    pub fn contains(&self, k: K) -> bool
    where
        usize: From<K>,
    {
        self.0.contains(usize::from(k) as u32)
    }

    pub fn insert(&mut self, k: K) -> bool
    where
        usize: From<K>,
    {
        self.0.insert(usize::from(k) as u32)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, K> {
        Iter(self.0.iter(), PhantomData)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    pub fn retains<F>(&mut self, mut f: F)
    where
        F: FnMut(K) -> bool,
        K: TryFrom<usize>,
    {
        let mut new = RoaringBitmap::new();

        for v in &self.0 {
            match K::try_from(v as usize) {
                Ok(k) => {
                    if f(k) {
                        new.insert(v);
                    }
                }
                Err(_) => unreachable!("Cannot convert to K"),
            };
        }

        self.0 = new;
    }
}

impl<K> BitAnd for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0, PhantomData)
    }
}

impl<K> BitAnd for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Set(&self.0 & &rhs.0, PhantomData)
    }
}

impl<K> BitAnd<Set<K>> for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitand(self, rhs: Set<K>) -> Self::Output {
        Set(&self.0 & rhs.0, PhantomData)
    }
}

impl<K> BitAnd<&Set<K>> for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitand(self, rhs: &Self) -> Self::Output {
        Self(&self.0 & &rhs.0, PhantomData)
    }
}

impl<K> BitAndAssign for Set<K> {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl<K> BitAndAssign<&Set<K>> for Set<K> {
    #[inline]
    fn bitand_assign(&mut self, rhs: &Self) {
        self.0 &= &rhs.0;
    }
}

impl<K> BitOr for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0, PhantomData)
    }
}

impl<K> BitOr for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Set(&self.0 | &rhs.0, PhantomData)
    }
}

impl<K> BitOr<Set<K>> for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitor(self, rhs: Set<K>) -> Self::Output {
        Set(&self.0 | rhs.0, PhantomData)
    }
}

impl<K> BitOr<&Set<K>> for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn bitor(self, rhs: &Self) -> Self::Output {
        Self(&self.0 | &rhs.0, PhantomData)
    }
}

impl<K> BitOrAssign for Set<K> {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl<K> BitOrAssign<&Set<K>> for Set<K> {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Self) {
        self.0 |= &rhs.0;
    }
}

impl<K> Clone for Set<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<K> Debug for Set<K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Set").field(&self.0).field(&self.1).finish()
    }
}

impl<K> Default for Set<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> FromIterator<K> for Set<K>
where
    usize: From<K>,
{
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        // collect into a vec to minimize the monomorphisation of the code
        let vec = iter
            .into_iter()
            .map(|v| usize::from(v) as u32)
            .collect::<Vec<_>>();

        Self(RoaringBitmap::from_iter(vec), PhantomData)
    }
}

impl<K> Gc for Set<K> {}

impl<K> IntoIterator for Set<K>
where
    K: TryFrom<usize>,
{
    type Item = K;
    type IntoIter = IntoIter<K>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter(), PhantomData)
    }
}

impl<'a, K> IntoIterator for &'a Set<K>
where
    K: TryFrom<usize>,
{
    type Item = K;
    type IntoIter = Iter<'a, K>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K> PartialEq for Set<K> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K> Sub for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0, PhantomData)
    }
}

impl<K> Sub for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Set(&self.0 - &rhs.0, PhantomData)
    }
}

impl<K> Sub<Set<K>> for &Set<K> {
    type Output = Set<K>;

    #[inline]
    fn sub(self, rhs: Set<K>) -> Self::Output {
        Set(&self.0 - rhs.0, PhantomData)
    }
}

impl<K> Sub<&Set<K>> for Set<K> {
    type Output = Set<K>;

    #[inline]
    fn sub(self, rhs: &Self) -> Self::Output {
        Self(&self.0 - &rhs.0, PhantomData)
    }
}

impl<K> SubAssign for Set<K> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<K> SubAssign<&Set<K>> for Set<K> {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        self.0 -= &rhs.0;
    }
}

pub struct IntoIter<K>(bitmap::IntoIter, PhantomData<K>);

impl<K> Iterator for IntoIter<K>
where
    K: TryFrom<usize>,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| match K::try_from(v as usize) {
            Ok(v) => v,
            Err(_) => unreachable!("Cannot convert to K"),
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct Iter<'a, K>(bitmap::Iter<'a>, PhantomData<K>);

impl<K> Iterator for Iter<'_, K>
where
    K: TryFrom<usize>,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| match K::try_from(v as usize) {
            Ok(v) => v,
            Err(_) => unreachable!("Cannot convert to K"),
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub(crate) fn empty<'a>() -> &'a RoaringBitmap {
    static CELL: OnceLock<RoaringBitmap> = OnceLock::new();
    CELL.get_or_init(RoaringBitmap::new)
}
