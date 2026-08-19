use crate::types::{Bucket, SArray, StringT, SuffixError};

fn has_high_bit(j: usize) -> bool {
    j > usize::MAX / 2
}

fn get_counts(t: &StringT, c: &mut Bucket) {
    c.iter_mut().for_each(|c| *c = 0);
    t.iter().for_each(|character| c[*character as usize] += 1);
}

fn get_buckets(c: &Bucket, b: &mut Bucket, _k: usize, end: bool) {
    let mut sum = 0;
    if end {
        b.iter_mut().enumerate().for_each(|(i, b_el)| {
            sum += c[i];
            *b_el = sum;
        });
    } else {
        b.iter_mut().enumerate().for_each(|(i, b_el)| {
            *b_el = sum;
            sum += c[i];
        });
    }
}

fn induce_sa(
    string: &StringT,
    suffix_array: &mut SArray,
    counts: &mut Bucket,
    buckets: &mut Bucket,
    n: usize,
    k: usize,
) {
    assert!(n <= suffix_array.len());
    get_counts(string, counts);
    get_buckets(counts, buckets, k, false);

    let mut c0;
    let mut j = n - 1;
    let mut c1 = string[j] as usize;
    let mut index = buckets[c1];
    suffix_array[index] = if j > 0 && (string[j - 1] as usize) < c1 {
        !j
    } else {
        j
    };
    index += 1;
    for i in 0..n {
        j = suffix_array[i];
        suffix_array[i] = !j;
        if !has_high_bit(j) && j > 0 {
            j -= 1;
            c0 = string[j] as usize;
            if c0 != c1 {
                buckets[c1] = index;
                c1 = c0;
                index = buckets[c1];
            }
            suffix_array[index] = if j > 0 && !has_high_bit(j) && (string[j - 1] as usize) < c1 {
                !j
            } else {
                j
            };
            index += 1;
        }
    }

    // Compute SA
    // XXX: true here.
    get_counts(string, counts);
    get_buckets(counts, buckets, k, true);
    c1 = 0;
    index = buckets[c1];
    for i in (0..n).rev() {
        j = suffix_array[i];
        if j > 0 && !has_high_bit(j) {
            j -= 1;
            c0 = string[j] as usize;
            if c0 != c1 {
                buckets[c1] = index;
                c1 = c0;
                index = buckets[c1];
            }
            index -= 1;
            suffix_array[index] = if j == 0 || (string[j - 1] as usize) > c1 {
                !j
            } else {
                j
            };
        } else {
            suffix_array[i] = !j;
        }
    }
}

fn compute_bwt(
    string: &StringT,
    suffix_array: &mut SArray,
    counts: &mut Bucket,
    buckets: &mut Bucket,
    n: usize,
    k: usize,
) -> usize {
    // TODO
    let mut pidx = 0;
    get_counts(string, counts);
    get_buckets(counts, buckets, k, false);
    let mut j = n - 1;
    let mut c1 = string[j] as usize;
    let mut c0;
    let mut index = buckets[c1];
    // bb = SA + B[c1 = T[j = n - 1]];
    // *bb++ = ((0 < j) && (T[j - 1] < c1)) ? ~j : j;
    suffix_array[index] = if j > 0 && (string[j - 1] as usize) < c1 {
        !j
    } else {
        j
    };
    index += 1;
    for i in 0..n {
        j = suffix_array[i];
        if j > 0 {
            j -= 1;
            c0 = string[j] as usize;
            suffix_array[i] = !c0;
            if c0 != c1 {
                buckets[c1] = index;
                c1 = c0;
                index = buckets[c1];
            }
            suffix_array[index] = if j > 0 && (string[j - 1] as usize) < c1 {
                !j
            } else {
                j
            };
            index += 1;
        } else if j != 0 {
            suffix_array[i] = !j;
        }
    }

    // Compute SA
    get_counts(string, counts);
    get_buckets(counts, buckets, k, true);
    c1 = 0;
    index = buckets[c1];
    for i in (0..n).rev() {
        j = suffix_array[i];
        if j > 0 {
            j -= 1;
            c0 = string[j] as usize;
            suffix_array[i] = c0;
            if c0 != c1 {
                buckets[c1] = index;
                c1 = c0;
                index = buckets[c1];
            }
            index -= 1;
            suffix_array[index] = if j > 0 && (string[j - 1] as usize) > c1 {
                !(string[j - 1] as usize)
            } else {
                j
            };
        } else if j != 0 {
            suffix_array[i] = !j;
        } else {
            pidx = i
        }
    }
    pidx
}

#[allow(clippy::many_single_char_names)]
fn suffixsort(
    string: &StringT,
    suffix_array: &mut SArray,
    fs: usize,
    n: usize,
    k: usize,
    is_bwt: bool,
) -> Result<usize, SuffixError> {
    let mut pidx = 0;
    let mut c0;

    let mut counts = vec![0; k];
    let mut buckets = vec![0; k];
    get_counts(string, &mut counts);
    get_buckets(&counts, &mut buckets, k, true);
    // stage 1:
    // reduce the problem by at least 1/2
    // sort all the S-substrings
    for item in suffix_array.iter_mut() {
        *item = 0;
    }
    let mut c_index = 0;
    let mut c1 = string[n - 1] as usize;
    for i in (0..n - 1).rev() {
        c0 = string[i] as usize;
        if c0 < c1 + c_index {
            c_index = 1;
        } else if c_index != 0 {
            buckets[c1] -= 1;
            suffix_array[buckets[c1]] = i + 1;
            c_index = 0;
        }
        c1 = c0;
    }
    induce_sa(string, suffix_array, &mut counts, &mut buckets, n, k);

    // compact all the sorted substrings into the first m items of SA
    // 2*m must be not larger than n (proveable)

    // TODO: This was in the parallel loop.
    let mut p;
    let mut j;
    let mut m = 0;
    for i in 0..n {
        p = suffix_array[i];
        c0 = string[p] as usize;
        if p > 0 && (string[p - 1] as usize) > c0 {
            // TODO overly complex. But fricking hard to get right.
            j = p + 1;
            if j < n {
                c1 = string[j] as usize;
            }
            while j < n && c0 == c1 {
                c1 = string[j] as usize;
                j += 1;
            }
            if j < n && c0 < c1 {
                suffix_array[m] = p;
                m += 1;
            }
        }
    }
    j = m + (n >> 1);
    for item in suffix_array.iter_mut().take(j).skip(m) {
        *item = 0;
    }

    /* store the length of all substrings */
    j = n;
    let mut c_index = 0;
    c1 = string[n - 1] as usize;
    for i in (0..n - 1).rev() {
        c0 = string[i] as usize;
        if c0 < c1 + c_index {
            c_index = 1;
        } else if c_index != 0 {
            suffix_array[m + ((i + 1) >> 1)] = j - i - 1;
            j = i + 1;
            c_index = 0;
        }
        c1 = c0;
    }

    /* find the lexicographic names of all substrings */
    let mut name = 0;
    let mut q = n;
    let mut qlen = 0;
    let mut plen;
    let mut diff;
    for i in 0..m {
        p = suffix_array[i];
        plen = suffix_array[m + (p >> 1)];
        diff = true;
        if plen == qlen {
            j = 0;
            while j < plen && string[p + j] == string[q + j] {
                j += 1;
            }
            if j == plen {
                diff = false;
            }
        }
        if diff {
            name += 1;
            q = p;
            qlen = plen;
        }
        suffix_array[m + (p >> 1)] = name;
    }
    /* stage 2: solve the reduced problem
    recurse if names are not yet unique */
    if name < m {
        let ra_index = n + fs - m;
        j = m - 1;
        let a = m + (n >> 1);
        for i in (m..a).rev() {
            if suffix_array[i] != 0 {
                suffix_array[ra_index + j] = suffix_array[i] - 1;
                // XXX: Bug underflow caught by Rust yeah (well cpp used i32)
                j = j.saturating_sub(1);
            }
        }
        // XXX: Could call transmute on SA to avoid allocation.
        // but it requires unsafe.
        let ra: Vec<u32> = suffix_array
            .iter()
            .skip(ra_index)
            .take(m)
            .map(|n| *n as u32)
            .collect();
        suffixsort(&ra, suffix_array, fs + n - m * 2, m, name, false)?;
        // let ra: &[char] =
        //     unsafe { std::mem::transmute::<&[usize], &[char]>(&sa[ra_index..ra_index + m]) };
        // suffixsort(ra, sa, fs + n - m * 2, m, name, false)?;
        j = m - 1;
        c_index = 0;
        c1 = string[n - 1] as usize;
        for i in (0..n - 1).rev() {
            c0 = string[i] as usize;
            if c0 < c1 + c_index {
                c_index = 1;
            } else if c_index != 0 {
                suffix_array[ra_index + j] = i + 1;
                c_index = 0;
                j = j.saturating_sub(1);
            }
            c1 = c0;
        }
        // get index in s
        for i in 0..m {
            suffix_array[i] = suffix_array[ra_index + suffix_array[i]];
        }
    }

    /* stage 3: induce the result for the original problem */
    /* put all left-most S characters into their buckets */
    get_counts(string, &mut counts);
    get_buckets(&counts, &mut buckets, k, true);
    for item in suffix_array.iter_mut().take(n).skip(m) {
        *item = 0;
    }
    for i in (0..m).rev() {
        j = suffix_array[i];
        suffix_array[i] = 0;
        if buckets[string[j] as usize] > 0 {
            buckets[string[j] as usize] -= 1;
            suffix_array[buckets[string[j] as usize]] = j;
        }
    }
    if is_bwt {
        pidx = compute_bwt(string, suffix_array, &mut counts, &mut buckets, n, k);
    } else {
        induce_sa(string, suffix_array, &mut counts, &mut buckets, n, k);
    }

    Ok(pidx)
}

pub fn saisxx(
    string: &StringT,
    suffix_array: &mut SArray,
    n: usize,
    k: usize,
) -> Result<(), SuffixError> {
    if n == 1 {
        suffix_array[0] = 0;
        return Ok(());
    }
    let fs = 0;
    suffixsort(string, suffix_array, fs, n, k, false)?;
    Ok(())
}
fn _saisxx_bwt(
    t: &StringT,
    u: &mut StringT,
    sa: &mut SArray,
    n: usize,
    k: usize,
) -> Result<usize, SuffixError> {
    if n <= 1 {
        if n == 1 {
            u[0] = t[0];
        }
        return Ok(n);
    }
    let mut pidx = suffixsort(t, sa, 0, n, k, true)?;
    u[0] = t[n - 1];
    for i in 0..pidx {
        u[i + 1] = sa[i] as u32;
    }
    for i in pidx + 1..n {
        u[i] = sa[i] as u32
    }
    pidx += 1;
    Ok(pidx)
}
