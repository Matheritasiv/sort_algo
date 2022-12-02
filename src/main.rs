use rand::Rng;
use std::thread::spawn;
use std::ops::Deref;
use std::marker::Send;
use std::cmp::PartialOrd;
use std::time::Instant;

//{{{ Bitonic sort
//{{{ Iterator `SortIndex`
struct SortIndex {
    start          : usize,
    end            : usize,
    bound          : usize,
    char_major_bit : u32,
    char_minor_bit : u32,
    char_major     : usize,
    char_minor     : usize,
    rev            : bool,
}

impl SortIndex {
    fn new(start0: usize, end: usize, bound: usize,
           char_major_bit: u32, char_minor_bit: u32, rev: bool) -> Self {
        assert!(char_major_bit > char_minor_bit);
        let char_minor = 1usize << char_minor_bit;
        let char_major = 1usize << char_major_bit;
        let mut start = start0;
        let pair = {
            let mut rx = start >> char_major_bit;
            let mut i = 1u32;
            loop {
                let rxs = rx.wrapping_shr(i);
                if rxs == 0 { break; }
                rx ^= rxs;
                i = i.wrapping_shl(1);
            }
            (rx & 1).wrapping_shl(char_minor_bit)
        };
        if start & char_minor ^ pair != 0 {
            if start & char_minor == 0 {
                start |= char_minor;
            } else {
                start = start.wrapping_add(1);
                let mut rx = start.wrapping_shr(char_major_bit);
                let mut i = 1u32;
                loop {
                    let rxs = rx.wrapping_shr(i);
                    if rxs == 0 { break; }
                    rx ^= rxs;
                    i = i.wrapping_shl(1);
                }
                start = start & !char_minor ^ (rx & 1).wrapping_shl(char_minor_bit);
            }
            if start <= start0 { start = end; }
        }
        SortIndex {
            start, end, bound,
            char_major_bit, char_minor_bit,
            char_major, char_minor,
            rev,
        }
    }
}

impl Iterator for SortIndex {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<(usize, usize)> {
        let ret = self.start;
        let retx = ret ^ self.char_minor;
        if ret >= self.end || ret >= self.bound || retx >= self.bound {
            return None;
        }
        let pair = ret & self.char_minor;
        let mut x = (ret | self.char_minor).wrapping_add(1);
        if (x ^ ret) & self.char_major != 0 {
            let mut rx = x.wrapping_shr(self.char_major_bit);
            let mut i = 1u32;
            loop {
                let rxs = rx.wrapping_shr(i);
                if rxs == 0 { break; }
                rx ^= rxs;
                i = i.wrapping_shl(1);
            }
            x ^= (rx & 1).wrapping_shl(self.char_minor_bit);
        } else {
            x = x & !self.char_minor ^ pair;
        }
        self.start = if x <= ret { self.end } else { x };
        return Some(if self.rev { (retx, ret) } else { (ret, retx) });
    }
}
//}}}
//{{{ Raw pointer wrapper
struct PtrWrapper<T>(*mut T);

impl<T> PtrWrapper<T> {
    fn new(ptr_data: *mut T) -> Self {
        PtrWrapper(ptr_data)
    }
}

impl<T> Deref for PtrWrapper<T> {
    type Target = *mut T;
    fn deref(&self) -> &*mut T { &self.0 }
}
impl<T> Clone for PtrWrapper<T> {
    fn clone(&self) -> Self { Self(self.0.clone()) }
}
impl<T> Copy for PtrWrapper<T> {}
unsafe impl<T> Send for PtrWrapper<T> {}
//}}}
//{{{ Bitonic sort, recursion
fn bitonic_r_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    fn bitonic_divide(n: usize) -> usize {
        let (mut ind, mut n) = (1usize, n - 1 >> 1);
        while n != 0 { n >>= 1; ind <<= 1; }
        ind
    }
    fn bitonic_merge<T>(data: &mut [T], rev: bool)
    where T: Copy + PartialOrd {
        if data.len() <= 1 { return; }
        let n = data.len();
        let ind = bitonic_divide(n);
        {
            let (data, data2) = data.split_at_mut(ind);
            let data1 = &mut data[.. n - ind];
            let (data1, data2) = if rev { (data2, data1) } else { (data1, data2) };
            for (x, y) in data1.iter_mut().zip(data2) {
                if *y < *x {
                    (*x, *y) = (*y, *x);
                }
            }
        }
        bitonic_merge(&mut data[.. ind], rev);
        bitonic_merge(&mut data[ind ..], rev);
    }
    fn bitonic_sort<T>(data: &mut [T], rev: bool)
    where T: Copy + PartialOrd {
        if data.len() <= 1 { return; }
        let ind = bitonic_divide(data.len());
        let (data1, data2) = data.split_at_mut(ind);
        bitonic_sort(data1, !rev);
        bitonic_sort(data2, rev);
        bitonic_merge(data, rev);
    }
    bitonic_sort(data, false);
}
//}}}
//{{{ Bitonic sort, recursion, parallel
fn bitonic_rp_sort<T>(data: &mut [T], t_depth: u32)
where T: Copy + PartialOrd + 'static {
    fn bitonic_divide(n: usize) -> usize {
        let (mut ind, mut n) = (1usize, n - 1 >> 1);
        while n != 0 { n >>= 1; ind <<= 1; }
        ind
    }
    fn bitonic_merge<T>(data: &mut [T], rev: bool)
    where T: Copy + PartialOrd {
        if data.len() <= 1 { return; }
        let n = data.len();
        let ind = bitonic_divide(n);
        {
            let (data, data2) = data.split_at_mut(ind);
            let data1 = &mut data[.. n - ind];
            let (data1, data2) = if rev { (data2, data1) } else { (data1, data2) };
            for (x, y) in data1.iter_mut().zip(data2) {
                if *y < *x {
                    (*x, *y) = (*y, *x);
                }
            }
        }
        bitonic_merge(&mut data[.. ind], rev);
        bitonic_merge(&mut data[ind ..], rev);
    }
    fn bitonic_sort<T>(count: u32, data: &mut [T], rev: bool)
    where T: Copy + PartialOrd + 'static {
        if data.len() <= 1 { return; }
        let len = data.len();
        let ind = bitonic_divide(len);
        if count == 0 {
            let (data1, data2) = data.split_at_mut(ind);
            bitonic_sort(0, data1, !rev);
            bitonic_sort(0, data2, rev);
        } else {
            let data = PtrWrapper::new(data.as_mut_ptr());
            let thread1 = spawn(move || bitonic_sort(count - 1, unsafe {
                std::slice::from_raw_parts_mut(*data, ind)
            }, !rev));
            let thread2 = spawn(move || bitonic_sort(count - 1, unsafe {
                std::slice::from_raw_parts_mut(data.add(ind), len - ind)
            }, rev));
            thread1.join().unwrap();
            thread2.join().unwrap();
        }
        bitonic_merge(data, rev);
    }
    bitonic_sort(t_depth, data, false);
}
//}}}
//{{{ Bitonic sort, iteration
fn bitonic_i_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    if data.len() <= 1 { return; }
    let n = data.len();
    let depth = {
        let (mut depth, mut n) = (1u32, n - 1 >> 1);
        while n != 0 { n >>= 1; depth += 1; }
        depth
    };
    let mut rev = depth & 1 == 0;
    for cnt in 1 ..= depth {
        for i in (0 .. cnt).rev() {
            for (ind1, ind2) in SortIndex::new(0, n, n, cnt, i, rev) {
                if data[ind2] < data[ind1] {
                    data.swap(ind1, ind2);
                }
            }
        }
        rev = !rev;
    }
}
//}}}
//{{{ Bitonic sort, iteration, parallel
fn bitonic_ip_sort<T>(data: &mut [T], t_depth: u32)
where T: Copy + PartialOrd + 'static {
    if data.len() <= 1 { return; }
    let n = data.len();
    let data = PtrWrapper::new(data.as_mut_ptr());
    let t_n = 1usize << t_depth;
    let depth = {
        let (mut depth, mut n) = (1u32, n - 1 >> 1);
        while n != 0 { n >>= 1; depth += 1; }
        depth
    };
    let chunk_depth = if depth > t_depth {
        depth.wrapping_sub(t_depth)
    } else { 0 };
    let chunk = 1usize << chunk_depth;
    let mut rev = depth & 1 == 0;
    for cnt in 1 ..= depth {
        for i in (0 .. cnt).rev() {
            let mut thread_list = vec![];
            for j in 0 .. t_n {
                thread_list.push(spawn(move || {
                    for (ind1, ind2) in SortIndex::new(
                        j * chunk, (j + 1) * chunk, n, cnt, i, rev) {
                        let (ind1, ind2) = (ind1 as isize, ind2 as isize);
                        unsafe {
                            if *data.offset(ind2) < *data.offset(ind1) {
                                (*data.offset(ind1), *data.offset(ind2)) =
                                    (*data.offset(ind2), *data.offset(ind1));
                            }
                        }
                    }
                }));
            }
            for t in thread_list {
                t.join().unwrap();
            }
        }
        rev = !rev;
    }
}
//}}}
//}}}
//{{{ Smooth sort (based on binary heap)
fn smooth_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    fn heap_rectify<T>(data: &mut [T], depth: u32, flag: Option<&[bool]>)
    where T: Copy + PartialOrd {
        let n = data.len();
        if n <= 1 { return; }
        let mut ind = n - 1;
        let ind_r = ind - 1;
        let mut delta = if depth > 0 {
            (1usize << depth - 1) - 1
        } else { 0 };
        let ind_l = ind_r - delta;
        let prev = match flag {
            Some(flag) if ind_l >= delta && {
                let v = data[ind_l - delta];
                if v > data[ind] && (delta == 0 ||
                        v > data[ind_l] && v > data[ind_r]) {
                    data.swap(ind_l - delta, ind);
                    true
                } else { false }
            } => {
                if flag[0] {
                    let mut result = None;
                    for (i, &fl) in flag.into_iter().enumerate().skip(1) {
                        if fl {
                            result = Some((
                                ind_l - delta,
                                depth + i as u32,
                                &flag[i ..],
                            ));
                            break;
                        }
                    }
                    result
                } else {
                    Some((ind_l - delta, depth, &flag[1 ..]))
                }
            },
            _ => None,
        };
        let v = data[ind];
        while delta > 0 {
            let mut ind_s = ind - 1;
            let ind_l = ind_s - delta;
            if data[ind_s] < data[ind_l] {
                ind_s = ind_l;
            }
            if v < data[ind_s] {
                data[ind] = data[ind_s];
                ind = ind_s;
                delta >>= 1;
            } else { break; }
        }
        data[ind] = v;
        if let Some((ind_prev, depth, flag)) = prev {
            heap_rectify(&mut data[..= ind_prev], depth, Some(flag));
        }
    }
    let n = data.len();
    let mut flag = [false; 64];
    let mut last_bit = 0usize;
    let mut m_bit = 0usize;
    for i in 0 .. n {
        if !flag[0] {
            flag[last_bit] = false;
            last_bit += 1;
            if !flag[last_bit] {
                (flag[0], flag[last_bit]) = (true, true);
            }
        } else if !flag[1] {
            flag[1] = true;
            last_bit = 1;
        } else {
            flag[0] = false;
        }
        heap_rectify(&mut data[..= i], last_bit as u32,
            if last_bit > m_bit {
                m_bit = last_bit;
                if n - i <= 1 << m_bit as u32 {
                    m_bit = 0;
                    if flag[0] {
                        Some(&flag[last_bit ..])
                    } else {
                        Some(&flag[last_bit - 1 ..])
                    }
                } else { None }
            } else { None }
        );
    }
    for i in (1 .. n).rev() {
        if last_bit > 1 {
            if flag[0] {
                flag[0] = false;
                flag[last_bit] = false;
            }
            last_bit -= 1;
            flag[last_bit] = true;
            heap_rectify(&mut data[..= i - (1usize << last_bit)],
                last_bit as u32, Some(&flag[last_bit ..]));
            heap_rectify(&mut data[.. i],
                last_bit as u32, Some(&flag[last_bit - 1 ..]));
        } else if flag[0] {
            for (i, &fl) in (&flag).into_iter().enumerate().skip(2) {
                if fl {
                    last_bit = i;
                    break;
                }
            }
            flag[1] = false;
        } else {
            flag[0] = true;
        }
    }
}
//}}}
//{{{ Weak heap sort
fn weak_heap_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    if data.len() <= 1 { return; }
    let n = data.len();
    let mut flags = vec![false; n];
    for mut ind in 1 .. n {
        while ind != 0 {
            let mut parent = ind;
            while parent & 1 == 0 {
                parent >>= 1;
            }
            parent >>= 1;
            if data[parent] < data[ind] {
                data.swap(ind, parent);
            }
            ind = parent;
        }
    }
    let mut v = data[0];
    for ind in (1 .. n).rev() {
        (v, data[ind]) = (data[ind], v);
        let mut indl = vec![];
        let mut index = 1;
        while index < ind {
            let flag = flags[index];
            indl.push(index);
            index <<= 1;
            if flag { index |= 1; }
        }
        for ind in indl.into_iter().rev() {
            if v < data[ind] {
                (v, data[ind]) = (data[ind], v);
                flags[ind] = !flags[ind];
            }
        }
    }
    data[0] = v;
}
//}}}
//{{{ Heap sort
fn heap_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    if data.len() <= 1 { return; }
    let n = data.len();
    for mut ind in 1 .. n {
        while ind != 0 {
            let parent = ind - 1 >> 1;
            if data[parent] < data[ind] {
                data.swap(ind, parent);
            }
            ind = parent;
        }
    }
    for ind in (1 .. n).rev() {
        let v = data[ind];
        data[ind] = data[0];
        let mut index = 0usize;
        loop {
            let mut index_s = index.wrapping_shl(1) + 1;
            let index_r = index_s + 1;
            if index_s >= ind {
                data[index] = v;
                break;
            }
            if index_r < ind && data[index_s] < data[index_r] {
                index_s = index_r;
            }
            if v < data[index_s] {
                data[index] = data[index_s];
                index = index_s;
            } else {
                data[index] = v;
                break;
            }
        }
    }
}
//}}}
//{{{ Quick sort
fn quick_sort<T>(data: &mut [T])
where T: Copy + PartialOrd {
    if data.len() <= 1 { return; }
    let (v, n) = (data[0], data.len());
    let (mut ind_l, mut ind_r) = (1, n - 1);
    loop {
        while ind_l < n && data[ind_l] <= v { ind_l += 1; }
        while ind_l < ind_r && data[ind_r] >= v { ind_r -= 1; }
        if ind_l >= ind_r { break; }
        data.swap(ind_l, ind_r);
    }
    data.swap(0, ind_l - 1);
    quick_sort(&mut data[.. ind_l - 1]);
    quick_sort(&mut data[ind_l ..]);
}
//}}}

//{{{ Main test
fn main() {
    //{{{ Generate random integers
    let mut rng = rand::thread_rng();
    let n = i64::MAX;
    let nums = (0 .. 1000000).map(|_| rng.gen_range(-n ..= n)).collect::<Vec<i64>>();
    //}}}
    //{{{ Quick sort
    let (elapsed_quick, result_quick) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        quick_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    println!("quick      : {}ms", elapsed_quick);
    //}}}
    //{{{ Bitonic sort, recursion
    let (elapsed_bitonic_r, result_bitonic_r) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        bitonic_r_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_quick, result_bitonic_r);
    println!("bitonic_r  : {}ms", elapsed_bitonic_r);
    //}}}
    //{{{ Bitonic sort, recursion, parallel
    let (elapsed_bitonic_rp, result_bitonic_rp) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        bitonic_rp_sort(&mut nums, 2);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_bitonic_r, result_bitonic_rp);
    println!("bitonic_rp : {}ms", elapsed_bitonic_rp);
    //}}}
    //{{{ Bitonic sort, iteration
    let (elapsed_bitonic_i, result_bitonic_i) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        bitonic_i_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_bitonic_rp, result_bitonic_i);
    println!("bitonic_i  : {}ms", elapsed_bitonic_i);
    //}}}
    //{{{ Bitonic sort, iteration, parallel
    let (elapsed_bitonic_ip, result_bitonic_ip) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        bitonic_ip_sort(&mut nums, 2);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_bitonic_i, result_bitonic_ip);
    println!("bitonic_ip : {}ms", elapsed_bitonic_ip);
    //}}}
    //{{{ Smooth sort
    let (elapsed_smooth, result_smooth) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        smooth_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_bitonic_ip, result_smooth);
    println!("smooth     : {}ms", elapsed_smooth);
    //}}}
    //{{{ Weak heap sort
    let (elapsed_weak_heap, result_weak_heap) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        weak_heap_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_smooth, result_weak_heap);
    println!("weak heap  : {}ms", elapsed_weak_heap);
    //}}}
    //{{{ Heap sort
    let (elapsed_heap, result_heap) = {
        let mut nums = nums.clone();
        let now = Instant::now();
        heap_sort(&mut nums);
        (now.elapsed().as_millis(), nums)
    };
    assert_eq!(result_weak_heap, result_heap);
    println!("heap       : {}ms", elapsed_heap);
    //}}}
}
//}}}
