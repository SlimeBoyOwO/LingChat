//! Small wrapper around sentencepiece's esaxx suffix array C++ library.
//! The previous version uses unsafe optimized c++ code.
//! There exists another implementation a bit slower (~2x slower) that uses
//! safe rust. It's a bit slower because it uses usize (mostly 64bit) instead of i32 (32bit).
//! But it does seems to fix a few OOB issues in the cpp version
//! (which were not observed during normal use but still.)

use std::convert::TryInto;
mod esa;
mod sais;
mod types;

use esa::esaxx_rs;
use types::SuffixError;

#[cfg(feature = "cc")]
extern "C" {
    fn esaxx_int32(
        // This is char32
        T: *const u32,
        SA: *mut i32,
        L: *mut i32,
        R: *mut i32,
        D: *mut i32,
        n: u32,
        k: u32,
        nodeNum: &mut u32,
    ) -> i32;
}

#[cfg(feature = "cc")]
fn esaxx(
    chars: &[char],
    sa: &mut [i32],
    l: &mut [i32],
    r: &mut [i32],
    d: &mut [i32],
    alphabet_size: u32,
    node_num: &mut u32,
) -> Result<(), SuffixError> {
    let n = chars.len();
    if sa.len() != n || l.len() != n || r.len() != n || d.len() != n {
        return Err(SuffixError::InvalidLength);
    }
    unsafe {
        let err = esaxx_int32(
            chars.as_ptr() as *const u32,
            sa.as_mut_ptr(),
            l.as_mut_ptr(),
            r.as_mut_ptr(),
            d.as_mut_ptr(),
            n.try_into().unwrap(),
            alphabet_size,
            node_num,
        );
        if err != 0 {
            return Err(SuffixError::Internal);
        }
    }
    Ok(())
}

pub struct SuffixIterator<'a, T> {
    i: usize,
    suffix: &'a Suffix<T>,
}

pub struct Suffix<T> {
    chars: Vec<char>,
    sa: Vec<T>,
    l: Vec<T>,
    r: Vec<T>,
    d: Vec<T>,
    node_num: usize,
}

/// Creates the suffix array and provides an iterator over its items (Rust version)
/// See [suffix](fn.suffix.html)
pub fn suffix_rs(string: &str) -> Result<Suffix<usize>, SuffixError> {
    let chars: Vec<_> = string.chars().collect();
    let n = chars.len();
    let mut sa = vec![0; n];
    let mut l = vec![0; n];
    let mut r = vec![0; n];
    let mut d = vec![0; n];
    let alphabet_size = 0x110000; // All UCS4 range.
    let node_num = esaxx_rs(
        &chars.iter().map(|c| *c as u32).collect::<Vec<_>>(),
        &mut sa,
        &mut l,
        &mut r,
        &mut d,
        alphabet_size,
    )?;
    Ok(Suffix {
        chars,
        sa,
        l,
        r,
        d,
        node_num,
    })
}

/// Creates the suffix array and provides an iterator over its items (c++ unsafe version)
///
/// Gives you an iterator over the suffixes of the input array and their count within
/// the input srtring.
#[cfg(feature = "cpp")]
pub fn suffix(string: &str) -> Result<Suffix<i32>, SuffixError> {
    let chars: Vec<_> = string.chars().collect();
    let n = chars.len();
    let mut sa = vec![0; n];
    let mut l = vec![0; n];
    let mut r = vec![0; n];
    let mut d = vec![0; n];
    let mut node_num = 0;
    let alphabet_size = 0x110000; // All UCS4 range.
    esaxx(
        &chars,
        &mut sa,
        &mut l,
        &mut r,
        &mut d,
        alphabet_size,
        &mut node_num,
    )?;
    Ok(Suffix {
        chars,
        sa,
        l,
        r,
        d,
        node_num: node_num.try_into()?,
    })
}

impl<T> Suffix<T> {
    pub fn iter(&self) -> SuffixIterator<'_, T> {
        SuffixIterator { i: 0, suffix: self }
    }
}

impl<'a> Iterator for SuffixIterator<'a, i32> {
    type Item = (&'a [char], u32);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.i;
        if index == self.suffix.node_num {
            None
        } else {
            let left: usize = self.suffix.l[index].try_into().ok()?;
            let offset: usize = self.suffix.sa[left].try_into().ok()?;
            let len: usize = self.suffix.d[index].try_into().ok()?;
            let freq: u32 = (self.suffix.r[index] - self.suffix.l[index])
                .try_into()
                .ok()?;
            self.i += 1;
            Some((&self.suffix.chars[offset..offset + len], freq))
        }
    }
}

impl<'a> Iterator for SuffixIterator<'a, usize> {
    type Item = (&'a [char], u32);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.i;
        if index == self.suffix.node_num {
            None
        } else {
            let left: usize = self.suffix.l[index];
            let offset: usize = self.suffix.sa[left];
            let len: usize = self.suffix.d[index];
            let freq: u32 = (self.suffix.r[index] - self.suffix.l[index])
                .try_into()
                .unwrap();
            self.i += 1;
            Some((&self.suffix.chars[offset..offset + len], freq))
        }
    }
}
