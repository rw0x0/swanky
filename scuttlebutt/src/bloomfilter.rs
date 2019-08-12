//! Implementation of a bloom filter.

use sha2::{Digest, Sha256};

/// Simple implementation of a Bloom Filter. Which is guaranteed to return 1 if an element
/// is in the set, but returns 1 with probability p (settable) if an item is not in the
/// set. Does not reveal what is in the set.
#[derive(Debug, PartialEq, PartialOrd)]
pub struct BloomFilter {
    bits: Vec<bool>,
    nhashes: usize,
}

impl BloomFilter {
    /// Create a new BloomFilter with `size` entries, using `nhashes` hash functions.
    pub fn new(size: usize, nhashes: usize) -> Self {
        BloomFilter {
            bits: vec![false; size],
            nhashes,
        }
    }

    /// Compute required expansion for false positive probability `p`.
    ///
    /// That is - if you plan to insert `n` items into the BloomFilter, and want a false
    /// positive probability of `p`, then you should set the BloomFilter size to
    /// `compute_expansion(p) * n`.
    pub fn compute_expansion(p: f64) -> f64 {
        -1.44 * p.log2()
    }

    /// Compute required number of hash functions for false positive probability `p`.
    pub fn compute_nhashes(p: f64) -> usize {
        (-p.log2()).ceil() as usize
    }

    /// Create a new BloomFilter with false positive probability `p` which can support up
    /// to `n` insertions.
    pub fn with_false_positive_prob(p: f64, n: usize) -> Self {
        Self::new((Self::compute_expansion(p) * n as f64).ceil() as usize, Self::compute_nhashes(p))
    }

    /// Get the number of bins in this BloomFilter.
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Get the number of hash functions in this BloomFilter.
    pub fn nhashes(&self) -> usize {
        self.nhashes
    }

    /// Get bloom filter bins.
    pub fn bins(&self) -> Vec<bool> {
        self.bits.clone()
    }

    /// Get bloom filter bins packed in bytes.
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = unsafe { std::mem::transmute::<u64, [u8;8]>( self.len() as u64 ) }.to_vec();
        let nbytes = (self.len() as f64 / 8.0).ceil() as usize;
        bytes.resize(8 + nbytes, 0);
        for i in 0..bytes.len() - 8 {
            for j in 0..8 {
                if 8*i + j >= self.len() {
                    break;
                }
                bytes[8 + i] |= (self.bits[8*i + j] as u8) << j;
            }
        }
        bytes
    }

    /// Create bloom filter from bytes.
    pub fn from_bytes(bytes: &[u8], nhashes: usize) -> Self {
        let mut size_bytes = [0; 8];
        for i in 0..8 {
            size_bytes[i] = bytes[i];
        }
        let (_, rest) = bytes.split_at(8);
        let size = unsafe { std::mem::transmute::<[u8;8], u64>(size_bytes) } as usize;
        println!("size={}", size);
        let mut bits = vec![false; size];
        for i in 0..rest.len() {
            for j in 0..8 {
                if 8*i + j >= size {
                    break;
                }
                bits[8*i + j] = ((rest[i] >> j) & 1) != 0;
            }
        }
        BloomFilter { bits, nhashes }
    }

    /// Compute the bin that this value would go to in a BloomFilter.
    ///
    /// Result must be modded by the actual size of the bloom filter to avoid out of
    /// bounds errors.
    pub fn bin<V: AsRef<[u8]>>(value: &V, hash_index: usize) -> usize {
        let mut bytes = unsafe { std::mem::transmute::<usize, [u8; 8]>(hash_index) }.to_vec();
        bytes.extend(value.as_ref());
        let hbytes = Sha256::digest(&bytes);
        let mut index_bytes = [0; 8];
        for (x, y) in hbytes.iter().zip(index_bytes.iter_mut()) {
            *y = *x;
        }
        unsafe { std::mem::transmute::<[u8; 8], usize>(index_bytes) }
    }

    /// Insert an item into the BloomFilter.
    pub fn insert<V: AsRef<[u8]>>(&mut self, value: &V) {
        for hash_index in 0..self.nhashes {
            let i = Self::bin(value, hash_index) % self.len();
            self.bits[i] = true;
        }
    }

    /// Check whether an item exists in the BloomFilter.
    pub fn contains<V: AsRef<[u8]>>(&mut self, value: &V) -> bool {
        (0..self.nhashes).all(|hash_index| {
            let i = Self::bin(value, hash_index) % self.len();
            self.bits[i]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AesRng, Block};
    use rand::Rng;

    #[test]
    fn test_bloom_filter_membership() {
        let mut rng = AesRng::new();
        let n = 1000;
        let nhashes = 3;
        let mut filter = BloomFilter::new(n, nhashes);
        for _ in 0..128 {
            let x = rng.gen::<Block>();
            filter.insert(&x);
            assert!(filter.contains(&x));
        }
        assert_eq!(filter, BloomFilter::from_bytes(&filter.as_bytes(), nhashes));
    }
}