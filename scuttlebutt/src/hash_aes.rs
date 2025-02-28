//! Implementations of correlation-robust hash functions (and their variants)
//! based on fixed-key AES.

use vectoreyes::{
    array_utils::{ArrayUnrolledExt, ArrayUnrolledOps, UnrollableArraySize},
    Aes128EncryptOnly, AesBlockCipher,
};

use crate::Block;

/// AES-based correlation-robust hash function.
///
/// This hash function supports the correlation-robust variants given in
/// <https://eprint.iacr.org/2019/074>.
pub struct AesHash {
    aes: Aes128EncryptOnly,
}

/// `AesHash` with a fixed key.
pub const AES_HASH: AesHash = AesHash {
    aes: Aes128EncryptOnly::FIXED_KEY,
};

impl AesHash {
    /// Initialize the hash function using `key`.
    #[inline]
    pub fn new(key: Block) -> Self {
        let aes = Aes128EncryptOnly::new_with_key(key);
        AesHash { aes }
    }

    /// Correlation-robust hash function for 128-bit inputs (cf.
    /// <https://eprint.iacr.org/2019/074>, §7.2).
    ///
    /// The function computes `π(x) ⊕ x`.
    #[inline]
    pub fn cr_hash(&self, _i: Block, x: Block) -> Block {
        self.aes.encrypt(x) ^ x
    }

    /// Tweakable circular correlation robust hash function (cf.
    /// <https://eprint.iacr.org/2019/074>, §7.4).
    ///
    /// The function computes `π(π(x) ⊕ i) ⊕ π(x)`.
    #[inline]
    pub fn tccr_hash(&self, i: Block, x: Block) -> Block {
        let y = self.aes.encrypt(x);
        let t = y ^ i;
        let z = self.aes.encrypt(t);
        y ^ z
    }

    /// Batch tweakable circular correlation robust hash function
    pub fn tccr_hash_many<const Q: usize>(&self, i: Block, xs: [Block; Q]) -> [Block; Q]
    where
        ArrayUnrolledOps: UnrollableArraySize<Q>,
    {
        let y = self.aes.encrypt_many(xs);
        let t = y.array_map(
            #[inline(always)]
            |x| x ^ i,
        );
        let z = self.aes.encrypt_many(t);
        y.array_zip(z).array_map(
            #[inline(always)]
            |(a, b)| a ^ b,
        )
    }
}
