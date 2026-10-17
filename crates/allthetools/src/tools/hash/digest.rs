//! MD5, SHA-1, SHA-256 and SHA-512, as RFC 1321 and FIPS 180-4 give them.
//!
//! These check that a file arrived whole, as `sha256sum` does. MD5 and SHA-1
//! are broken for signatures, and they are here only to check the old
//! lists of checksums that still name them.
//!
//! This module makes no system call, so its tests run on every host.

use std::fmt;

/// The algorithms that `att hash` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    /// The algorithm of a digest with this many hex digits.
    pub fn from_hex_len(len: usize) -> Option<Algorithm> {
        match len {
            32 => Some(Algorithm::Md5),
            40 => Some(Algorithm::Sha1),
            64 => Some(Algorithm::Sha256),
            128 => Some(Algorithm::Sha512),
            _ => None,
        }
    }

    /// The algorithm of a tag of the BSD form, such as `SHA256 (file) = ...`.
    pub fn from_tag(tag: &str) -> Option<Algorithm> {
        match tag.to_ascii_uppercase().replace('-', "").as_str() {
            "MD5" => Some(Algorithm::Md5),
            "SHA1" => Some(Algorithm::Sha1),
            "SHA256" => Some(Algorithm::Sha256),
            "SHA512" => Some(Algorithm::Sha512),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Algorithm::Md5 => "MD5",
            Algorithm::Sha1 => "SHA1",
            Algorithm::Sha256 => "SHA256",
            Algorithm::Sha512 => "SHA512",
        }
    }

    pub fn hasher(self) -> Hasher {
        match self {
            Algorithm::Md5 => Hasher::Md5(Md5::new()),
            Algorithm::Sha1 => Hasher::Sha1(Sha1::new()),
            Algorithm::Sha256 => Hasher::Sha256(Sha256::new()),
            Algorithm::Sha512 => Hasher::Sha512(Sha512::new()),
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.tag().to_ascii_lowercase())
    }
}

/// One running hash of any algorithm.
pub enum Hasher {
    Md5(Md5),
    Sha1(Sha1),
    Sha256(Sha256),
    Sha512(Sha512),
}

impl Hasher {
    pub fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::Md5(h) => h.0.update(data, md5_block),
            Hasher::Sha1(h) => h.0.update(data, sha1_block),
            Hasher::Sha256(h) => h.0.update(data, sha256_block),
            Hasher::Sha512(h) => h.update(data),
        }
    }

    /// The digest as lowercase hex digits.
    pub fn hex(self) -> String {
        let bytes = match self {
            Hasher::Md5(h) => h.finish(),
            Hasher::Sha1(h) => h.finish(),
            Hasher::Sha256(h) => h.finish(),
            Hasher::Sha512(h) => h.finish(),
        };
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// The buffer of a hash with blocks of 64 bytes, and its state of `N` words.
struct Block64<const N: usize> {
    state: [u32; N],
    buffer: [u8; 64],
    filled: usize,
    length: u64,
}

impl<const N: usize> Block64<N> {
    fn new(state: [u32; N]) -> Self {
        Block64 {
            state,
            buffer: [0; 64],
            filled: 0,
            length: 0,
        }
    }

    fn update(&mut self, mut data: &[u8], block: fn(&mut [u32; N], &[u8; 64])) {
        self.length = self.length.wrapping_add(data.len() as u64);
        if self.filled > 0 {
            let take = (64 - self.filled).min(data.len());
            self.buffer[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled < 64 {
                return;
            }
            let full = self.buffer;
            block(&mut self.state, &full);
            self.filled = 0;
        }
        let mut chunks = data.chunks_exact(64);
        for chunk in &mut chunks {
            block(&mut self.state, chunk.try_into().expect("64 bytes"));
        }
        let rest = chunks.remainder();
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }

    /// Add the padding: a one bit, zeros, and the length in bits, in the
    /// byte order that the algorithm uses.
    fn pad(&mut self, little: bool, block: fn(&mut [u32; N], &[u8; 64])) {
        let bits = self.length.wrapping_mul(8);
        let mut tail = vec![0x80u8];
        let used = (self.filled + 1) % 64;
        let zeros = if used <= 56 { 56 - used } else { 120 - used };
        tail.extend(std::iter::repeat_n(0, zeros));
        tail.extend(if little {
            bits.to_le_bytes()
        } else {
            bits.to_be_bytes()
        });
        let length = self.length;
        self.update(&tail, block);
        self.length = length;
    }
}

pub struct Md5(Block64<4>);
pub struct Sha1(Block64<5>);
pub struct Sha256(Block64<8>);

impl Md5 {
    fn new() -> Self {
        Md5(Block64::new([
            0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476,
        ]))
    }

    fn finish(mut self) -> Vec<u8> {
        self.0.pad(true, md5_block);
        self.0.state.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

impl Sha1 {
    fn new() -> Self {
        Sha1(Block64::new([
            0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0,
        ]))
    }

    fn finish(mut self) -> Vec<u8> {
        self.0.pad(false, sha1_block);
        self.0.state.iter().flat_map(|w| w.to_be_bytes()).collect()
    }
}

impl Sha256 {
    fn new() -> Self {
        Sha256(Block64::new([
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ]))
    }

    fn finish(mut self) -> Vec<u8> {
        self.0.pad(false, sha256_block);
        self.0.state.iter().flat_map(|w| w.to_be_bytes()).collect()
    }
}

const MD5_SHIFT: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

fn md5_block(state: &mut [u32; 4], block: &[u8; 64]) {
    let m: Vec<u32> = block
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let [mut a, mut b, mut c, mut d] = *state;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let next = a
            .wrapping_add(f)
            .wrapping_add(MD5_K[i])
            .wrapping_add(m[g])
            .rotate_left(MD5_SHIFT[i]);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(next);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d]) {
        *s = s.wrapping_add(v);
    }
}

fn sha1_block(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (i, c) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, word) in w.iter().enumerate() {
        let (f, k) = match i / 20 {
            0 => ((b & c) | (!b & d), 0x5a827999),
            1 => (b ^ c ^ d, 0x6ed9eba1),
            2 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let t = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = t;
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e]) {
        *s = s.wrapping_add(v);
    }
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256_block(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, c) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// SHA-512, which has blocks of 128 bytes and words of 64 bits.
pub struct Sha512 {
    state: [u64; 8],
    buffer: [u8; 128],
    filled: usize,
    length: u128,
}

const SHA512_K: [u64; 80] = [
    0x428a2f98d728ae22,
    0x7137449123ef65cd,
    0xb5c0fbcfec4d3b2f,
    0xe9b5dba58189dbbc,
    0x3956c25bf348b538,
    0x59f111f1b605d019,
    0x923f82a4af194f9b,
    0xab1c5ed5da6d8118,
    0xd807aa98a3030242,
    0x12835b0145706fbe,
    0x243185be4ee4b28c,
    0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f,
    0x80deb1fe3b1696b1,
    0x9bdc06a725c71235,
    0xc19bf174cf692694,
    0xe49b69c19ef14ad2,
    0xefbe4786384f25e3,
    0x0fc19dc68b8cd5b5,
    0x240ca1cc77ac9c65,
    0x2de92c6f592b0275,
    0x4a7484aa6ea6e483,
    0x5cb0a9dcbd41fbd4,
    0x76f988da831153b5,
    0x983e5152ee66dfab,
    0xa831c66d2db43210,
    0xb00327c898fb213f,
    0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2,
    0xd5a79147930aa725,
    0x06ca6351e003826f,
    0x142929670a0e6e70,
    0x27b70a8546d22ffc,
    0x2e1b21385c26c926,
    0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df,
    0x650a73548baf63de,
    0x766a0abb3c77b2a8,
    0x81c2c92e47edaee6,
    0x92722c851482353b,
    0xa2bfe8a14cf10364,
    0xa81a664bbc423001,
    0xc24b8b70d0f89791,
    0xc76c51a30654be30,
    0xd192e819d6ef5218,
    0xd69906245565a910,
    0xf40e35855771202a,
    0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8,
    0x1e376c085141ab53,
    0x2748774cdf8eeb99,
    0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63,
    0x4ed8aa4ae3418acb,
    0x5b9cca4f7763e373,
    0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc,
    0x78a5636f43172f60,
    0x84c87814a1f0ab72,
    0x8cc702081a6439ec,
    0x90befffa23631e28,
    0xa4506cebde82bde9,
    0xbef9a3f7b2c67915,
    0xc67178f2e372532b,
    0xca273eceea26619c,
    0xd186b8c721c0c207,
    0xeada7dd6cde0eb1e,
    0xf57d4f7fee6ed178,
    0x06f067aa72176fba,
    0x0a637dc5a2c898a6,
    0x113f9804bef90dae,
    0x1b710b35131c471b,
    0x28db77f523047d84,
    0x32caab7b40c72493,
    0x3c9ebe0a15c9bebc,
    0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6,
    0x597f299cfc657e2a,
    0x5fcb6fab3ad6faec,
    0x6c44198c4a475817,
];

impl Sha512 {
    fn new() -> Self {
        Sha512 {
            state: [
                0x6a09e667f3bcc908,
                0xbb67ae8584caa73b,
                0x3c6ef372fe94f82b,
                0xa54ff53a5f1d36f1,
                0x510e527fade682d1,
                0x9b05688c2b3e6c1f,
                0x1f83d9abfb41bd6b,
                0x5be0cd19137e2179,
            ],
            buffer: [0; 128],
            filled: 0,
            length: 0,
        }
    }

    fn block(state: &mut [u64; 8], block: &[u8]) {
        let mut w = [0u64; 80];
        for (i, c) in block.chunks_exact(8).enumerate() {
            w[i] = u64::from_be_bytes(c.try_into().expect("8 bytes"));
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA512_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u128);
        if self.filled > 0 {
            let take = (128 - self.filled).min(data.len());
            self.buffer[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled < 128 {
                return;
            }
            let full = self.buffer;
            Sha512::block(&mut self.state, &full);
            self.filled = 0;
        }
        let mut chunks = data.chunks_exact(128);
        for chunk in &mut chunks {
            Sha512::block(&mut self.state, chunk);
        }
        let rest = chunks.remainder();
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }

    fn finish(mut self) -> Vec<u8> {
        let bits = self.length.wrapping_mul(8);
        let used = (self.filled + 1) % 128;
        let zeros = if used <= 112 { 112 - used } else { 240 - used };
        let mut tail = vec![0x80u8];
        tail.extend(std::iter::repeat_n(0, zeros));
        tail.extend(bits.to_be_bytes());
        self.update(&tail);
        self.state.iter().flat_map(|w| w.to_be_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(algorithm: Algorithm, data: &[u8]) -> String {
        let mut h = algorithm.hasher();
        h.update(data);
        h.hex()
    }

    // The vectors of RFC 1321, FIPS 180-4 and the examples of NIST.
    #[test]
    fn md5_matches_rfc_1321() {
        assert_eq!(hex(Algorithm::Md5, b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(
            hex(Algorithm::Md5, b"abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(
            hex(
                Algorithm::Md5,
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            ),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn sha1_matches_fips_180() {
        assert_eq!(
            hex(Algorithm::Sha1, b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(
                Algorithm::Sha1,
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            ),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn sha256_matches_fips_180() {
        assert_eq!(
            hex(Algorithm::Sha256, b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(Algorithm::Sha256, b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(
                Algorithm::Sha256,
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            ),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn sha512_matches_fips_180() {
        assert_eq!(
            hex(Algorithm::Sha512, b"abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            hex(Algorithm::Sha512, b""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn a_million_a_in_parts_matches_fips_180() {
        // The input arrives in parts of odd sizes, so each path of the
        // buffer runs.
        let data = vec![b'a'; 1_000_000];
        let mut h = Algorithm::Sha256.hasher();
        for part in data.chunks(997) {
            h.update(part);
        }
        assert_eq!(
            h.hex(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        let mut h = Algorithm::Sha1.hasher();
        for part in data.chunks(61) {
            h.update(part);
        }
        assert_eq!(h.hex(), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
    }

    #[test]
    fn each_length_near_a_block_gives_a_new_digest() {
        // The padding changes at 55, 56 and 64 bytes. Each length must give
        // a digest of its own.
        let mut seen = std::collections::HashSet::new();
        for len in 50..70 {
            assert!(seen.insert(hex(Algorithm::Sha256, &vec![1u8; len])));
        }
    }

    #[test]
    fn the_algorithm_follows_the_length_and_the_tag() {
        assert_eq!(Algorithm::from_hex_len(64), Some(Algorithm::Sha256));
        assert_eq!(Algorithm::from_hex_len(10), None);
        assert_eq!(Algorithm::from_tag("SHA-256"), Some(Algorithm::Sha256));
        assert_eq!(Algorithm::from_tag("md5"), Some(Algorithm::Md5));
    }
}
