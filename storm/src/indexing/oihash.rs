use fxhash::FxHasher;
use std::hash::Hasher;

/// An order indepdendant hasher. This is not safe for duplicate items.
pub struct OiHash(u64);

impl OiHash {
    #[inline]
    pub fn new() -> Self {
        Self(0)
    }
}

impl Hasher for OiHash {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut h = FxHasher::default();
        h.write(bytes);
        let hash = h.finish();
        self.0 ^= hash;
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0 ^= i as u64;
    }
}

impl Default for OiHash {
    #[inline]
    fn default() -> Self {
        Self(0)
    }
}
