use core::result::Result::{Ok, Err};

#[test]
fn test_position() {
    let b = [1, 2, 3, 5, 5];
    assert!(b.iter().position(|&v| v == 9) == None);
    assert!(b.iter().position(|&v| v == 5) == Some(3));
    assert!(b.iter().position(|&v| v == 3) == Some(2));
    assert!(b.iter().position(|&v| v == 0) == None);
}

#[test]
fn test_rposition() {
    let b = [1, 2, 3, 5, 5];
    assert!(b.iter().rposition(|&v| v == 9) == None);
    assert!(b.iter().rposition(|&v| v == 5) == Some(4));
    assert!(b.iter().rposition(|&v| v == 3) == Some(2));
    assert!(b.iter().rposition(|&v| v == 0) == None);
}

#[test]
fn test_binary_search() {
    let b: [i32; 0] = [];
    assert_eq!(b.binary_search(&5), Err(0));

    let b = [4];
    assert_eq!(b.binary_search(&3), Err(0));
    assert_eq!(b.binary_search(&4), Ok(0));
    assert_eq!(b.binary_search(&5), Err(1));

    let b = [1, 2, 4, 6, 8, 9];
    assert_eq!(b.binary_search(&5), Err(3));
    assert_eq!(b.binary_search(&6), Ok(3));
    assert_eq!(b.binary_search(&7), Err(4));
    assert_eq!(b.binary_search(&8), Ok(4));

    let b = [1, 2, 4, 5, 6, 8];
    assert_eq!(b.binary_search(&9), Err(6));

    let b = [1, 2, 4, 6, 7, 8, 9];
    assert_eq!(b.binary_search(&6), Ok(3));
    assert_eq!(b.binary_search(&5), Err(3));
    assert_eq!(b.binary_search(&8), Ok(5));

    let b = [1, 2, 4, 5, 6, 8, 9];
    assert_eq!(b.binary_search(&7), Err(5));
    assert_eq!(b.binary_search(&0), Err(0));

    let b = [1, 3, 3, 3, 7];
    assert_eq!(b.binary_search(&0), Err(0));
    assert_eq!(b.binary_search(&1), Ok(0));
    assert_eq!(b.binary_search(&2), Err(1));
    assert!(match b.binary_search(&3) { Ok(1..=3) => true, _ => false });
    assert!(match b.binary_search(&3) { Ok(1..=3) => true, _ => false });
    assert_eq!(b.binary_search(&4), Err(4));
    assert_eq!(b.binary_search(&5), Err(4));
    assert_eq!(b.binary_search(&6), Err(4));
    assert_eq!(b.binary_search(&7), Ok(4));
    assert_eq!(b.binary_search(&8), Err(5));
}

#[test]
// Test implementation specific behavior when finding equivalent elements.
// It is ok to break this test but when you do a crater run is highly advisable.
fn test_binary_search_implementation_details() {
    let b = [1, 1, 2, 2, 3, 3, 3];
    assert_eq!(b.binary_search(&1), Ok(1));
    assert_eq!(b.binary_search(&2), Ok(3));
    assert_eq!(b.binary_search(&3), Ok(6));
    let b = [1, 1, 1, 1, 1, 3, 3, 3, 3];
    assert_eq!(b.binary_search(&1), Ok(4));
    assert_eq!(b.binary_search(&3), Ok(8));
    let b = [1, 1, 1, 1, 3, 3, 3, 3, 3];
    assert_eq!(b.binary_search(&1), Ok(3));
    assert_eq!(b.binary_search(&3), Ok(8));
}

#[test]
fn test_iterator_nth() {
    let v: &[_] = &[0, 1, 2, 3, 4];
    for i in 0..v.len() {
        assert_eq!(v.iter().nth(i).unwrap(), &v[i]);
    }
    assert_eq!(v.iter().nth(v.len()), None);

    let mut iter = v.iter();
    assert_eq!(iter.nth(2).unwrap(), &v[2]);
    assert_eq!(iter.nth(1).unwrap(), &v[4]);
}

#[test]
fn test_iterator_last() {
    let v: &[_] = &[0, 1, 2, 3, 4];
    assert_eq!(v.iter().last().unwrap(), &4);
    assert_eq!(v[..1].iter().last().unwrap(), &0);
}

#[test]
fn test_iterator_count() {
    let v: &[_] = &[0, 1, 2, 3, 4];
    assert_eq!(v.iter().count(), 5);

    let mut iter2 = v.iter();
    iter2.next();
    iter2.next();
    assert_eq!(iter2.count(), 3);
}

#[test]
fn test_chunks_count() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.chunks(3);
    assert_eq!(c.count(), 2);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.chunks(2);
    assert_eq!(c2.count(), 3);

    let v3: &[i32] = &[];
    let c3 = v3.chunks(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_chunks_nth() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.chunks(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[4, 5]);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.chunks(3);
    assert_eq!(c2.nth(1).unwrap(), &[3, 4]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_chunks_nth_back() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.chunks(2);
    assert_eq!(c.nth_back(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[0, 1]);
    assert_eq!(c.next(), None);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.chunks(3);
    assert_eq!(c2.nth_back(1).unwrap(), &[0, 1, 2]);
    assert_eq!(c2.next(), None);
    assert_eq!(c2.next_back(), None);

    let v3: &[i32] = &[0, 1, 2, 3, 4];
    let mut c3 = v3.chunks(10);
    assert_eq!(c3.nth_back(0).unwrap(), &[0, 1, 2, 3, 4]);
    assert_eq!(c3.next(), None);

    let v4: &[i32] = &[0, 1, 2];
    let mut c4 = v4.chunks(10);
    assert_eq!(c4.nth_back(1_000_000_000usize), None);
}

#[test]
fn test_chunks_last() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.chunks(2);
    assert_eq!(c.last().unwrap()[1], 5);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.chunks(2);
    assert_eq!(c2.last().unwrap()[0], 4);
}

#[test]
fn test_chunks_zip() {
    let v1: &[i32] = &[0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    let res = v1.chunks(2)
        .zip(v2.chunks(2))
        .map(|(a, b)| a.iter().sum::<i32>() + b.iter().sum::<i32>())
        .collect::<Vec<_>>();
    assert_eq!(res, vec![14, 22, 14]);
}

#[test]
fn test_chunks_mut_count() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.chunks_mut(3);
    assert_eq!(c.count(), 2);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.chunks_mut(2);
    assert_eq!(c2.count(), 3);

    let v3: &mut [i32] = &mut [];
    let c3 = v3.chunks_mut(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_chunks_mut_nth() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.chunks_mut(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let mut c2 = v2.chunks_mut(3);
    assert_eq!(c2.nth(1).unwrap(), &[3, 4]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_chunks_mut_last() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.chunks_mut(2);
    assert_eq!(c.last().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.chunks_mut(2);
    assert_eq!(c2.last().unwrap(), &[4]);
}

#[test]
fn test_chunks_mut_zip() {
    let v1: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    for (a, b) in v1.chunks_mut(2).zip(v2.chunks(2)) {
        let sum = b.iter().sum::<i32>();
        for v in a {
            *v += sum;
        }
    }
    assert_eq!(v1, [13, 14, 19, 20, 14]);
}

#[test]
fn test_chunks_exact_count() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.chunks_exact(3);
    assert_eq!(c.count(), 2);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.chunks_exact(2);
    assert_eq!(c2.count(), 2);

    let v3: &[i32] = &[];
    let c3 = v3.chunks_exact(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_chunks_exact_nth() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.chunks_exact(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[4, 5]);

    let v2: &[i32] = &[0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.chunks_exact(3);
    assert_eq!(c2.nth(1).unwrap(), &[3, 4, 5]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_chunks_exact_last() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.chunks_exact(2);
    assert_eq!(c.last().unwrap(), &[4, 5]);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.chunks_exact(2);
    assert_eq!(c2.last().unwrap(), &[2, 3]);
}

#[test]
fn test_chunks_exact_remainder() {
    let v: &[i32] = &[0, 1, 2, 3, 4];
    let c = v.chunks_exact(2);
    assert_eq!(c.remainder(), &[4]);
}

#[test]
fn test_chunks_exact_zip() {
    let v1: &[i32] = &[0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    let res = v1.chunks_exact(2)
        .zip(v2.chunks_exact(2))
        .map(|(a, b)| a.iter().sum::<i32>() + b.iter().sum::<i32>())
        .collect::<Vec<_>>();
    assert_eq!(res, vec![14, 22]);
}

#[test]
fn test_chunks_exact_mut_count() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.chunks_exact_mut(3);
    assert_eq!(c.count(), 2);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.chunks_exact_mut(2);
    assert_eq!(c2.count(), 2);

    let v3: &mut [i32] = &mut [];
    let c3 = v3.chunks_exact_mut(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_chunks_exact_mut_nth() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.chunks_exact_mut(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.chunks_exact_mut(3);
    assert_eq!(c2.nth(1).unwrap(), &[3, 4, 5]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_chunks_exact_mut_last() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.chunks_exact_mut(2);
    assert_eq!(c.last().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.chunks_exact_mut(2);
    assert_eq!(c2.last().unwrap(), &[2, 3]);
}

#[test]
fn test_chunks_exact_mut_remainder() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c = v.chunks_exact_mut(2);
    assert_eq!(c.into_remainder(), &[4]);
}

#[test]
fn test_chunks_exact_mut_zip() {
    let v1: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    for (a, b) in v1.chunks_exact_mut(2).zip(v2.chunks_exact(2)) {
        let sum = b.iter().sum::<i32>();
        for v in a {
            *v += sum;
        }
    }
    assert_eq!(v1, [13, 14, 19, 20, 4]);
}

#[test]
fn test_rchunks_count() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.rchunks(3);
    assert_eq!(c.count(), 2);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.rchunks(2);
    assert_eq!(c2.count(), 3);

    let v3: &[i32] = &[];
    let c3 = v3.rchunks(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_rchunks_nth() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[0, 1]);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.rchunks(3);
    assert_eq!(c2.nth(1).unwrap(), &[0, 1]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_nth_back() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks(2);
    assert_eq!(c.nth_back(1).unwrap(), &[2, 3]);
    assert_eq!(c.next_back().unwrap(), &[4, 5]);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.rchunks(3);
    assert_eq!(c2.nth_back(1).unwrap(), &[2, 3, 4]);
    assert_eq!(c2.next_back(), None);
}

#[test]
fn test_rchunks_last() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.rchunks(2);
    assert_eq!(c.last().unwrap()[1], 1);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.rchunks(2);
    assert_eq!(c2.last().unwrap()[0], 0);
}

#[test]
fn test_rchunks_zip() {
    let v1: &[i32] = &[0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    let res = v1.rchunks(2)
        .zip(v2.rchunks(2))
        .map(|(a, b)| a.iter().sum::<i32>() + b.iter().sum::<i32>())
        .collect::<Vec<_>>();
    assert_eq!(res, vec![26, 18, 6]);
}

#[test]
fn test_rchunks_mut_count() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.rchunks_mut(3);
    assert_eq!(c.count(), 2);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.rchunks_mut(2);
    assert_eq!(c2.count(), 3);

    let v3: &mut [i32] = &mut [];
    let c3 = v3.rchunks_mut(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_rchunks_mut_nth() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_mut(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[0, 1]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let mut c2 = v2.rchunks_mut(3);
    assert_eq!(c2.nth(1).unwrap(), &[0, 1]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_mut_nth_back() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_mut(2);
    assert_eq!(c.nth_back(1).unwrap(), &[2, 3]);
    assert_eq!(c.next_back().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let mut c2 = v2.rchunks_mut(3);
    assert_eq!(c2.nth_back(1).unwrap(), &[2, 3, 4]);
    assert_eq!(c2.next_back(), None);
}

#[test]
fn test_rchunks_mut_last() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.rchunks_mut(2);
    assert_eq!(c.last().unwrap(), &[0, 1]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.rchunks_mut(2);
    assert_eq!(c2.last().unwrap(), &[0]);
}

#[test]
fn test_rchunks_mut_zip() {
    let v1: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    for (a, b) in v1.rchunks_mut(2).zip(v2.rchunks(2)) {
        let sum = b.iter().sum::<i32>();
        for v in a {
            *v += sum;
        }
    }
    assert_eq!(v1, [6, 16, 17, 22, 23]);
}

#[test]
fn test_rchunks_exact_count() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.rchunks_exact(3);
    assert_eq!(c.count(), 2);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.rchunks_exact(2);
    assert_eq!(c2.count(), 2);

    let v3: &[i32] = &[];
    let c3 = v3.rchunks_exact(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_rchunks_exact_nth() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_exact(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[0, 1]);

    let v2: &[i32] = &[0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.rchunks_exact(3);
    assert_eq!(c2.nth(1).unwrap(), &[1, 2, 3]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_exact_nth_back() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_exact(2);
    assert_eq!(c.nth_back(1).unwrap(), &[2, 3]);
    assert_eq!(c.next_back().unwrap(), &[4, 5]);

    let v2: &[i32] = &[0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.rchunks_exact(3);
    assert_eq!(c2.nth_back(1).unwrap(), &[4, 5, 6]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_exact_last() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.rchunks_exact(2);
    assert_eq!(c.last().unwrap(), &[0, 1]);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.rchunks_exact(2);
    assert_eq!(c2.last().unwrap(), &[1, 2]);
}

#[test]
fn test_rchunks_exact_remainder() {
    let v: &[i32] = &[0, 1, 2, 3, 4];
    let c = v.rchunks_exact(2);
    assert_eq!(c.remainder(), &[0]);
}

#[test]
fn test_rchunks_exact_zip() {
    let v1: &[i32] = &[0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    let res = v1.rchunks_exact(2)
        .zip(v2.rchunks_exact(2))
        .map(|(a, b)| a.iter().sum::<i32>() + b.iter().sum::<i32>())
        .collect::<Vec<_>>();
    assert_eq!(res, vec![26, 18]);
}

#[test]
fn test_rchunks_exact_mut_count() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.rchunks_exact_mut(3);
    assert_eq!(c.count(), 2);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.rchunks_exact_mut(2);
    assert_eq!(c2.count(), 2);

    let v3: &mut [i32] = &mut [];
    let c3 = v3.rchunks_exact_mut(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_rchunks_exact_mut_nth() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_exact_mut(2);
    assert_eq!(c.nth(1).unwrap(), &[2, 3]);
    assert_eq!(c.next().unwrap(), &[0, 1]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.rchunks_exact_mut(3);
    assert_eq!(c2.nth(1).unwrap(), &[1, 2, 3]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_exact_mut_nth_back() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let mut c = v.rchunks_exact_mut(2);
    assert_eq!(c.nth_back(1).unwrap(), &[2, 3]);
    assert_eq!(c.next_back().unwrap(), &[4, 5]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4, 5, 6];
    let mut c2 = v2.rchunks_exact_mut(3);
    assert_eq!(c2.nth_back(1).unwrap(), &[4, 5, 6]);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_rchunks_exact_mut_last() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4, 5];
    let c = v.rchunks_exact_mut(2);
    assert_eq!(c.last().unwrap(), &[0, 1]);

    let v2: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c2 = v2.rchunks_exact_mut(2);
    assert_eq!(c2.last().unwrap(), &[1, 2]);
}

#[test]
fn test_rchunks_exact_mut_remainder() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let c = v.rchunks_exact_mut(2);
    assert_eq!(c.into_remainder(), &[0]);
}

#[test]
fn test_rchunks_exact_mut_zip() {
    let v1: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    for (a, b) in v1.rchunks_exact_mut(2).zip(v2.rchunks_exact(2)) {
        let sum = b.iter().sum::<i32>();
        for v in a {
            *v += sum;
        }
    }
    assert_eq!(v1, [0, 16, 17, 22, 23]);
}

#[test]
fn test_windows_count() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.windows(3);
    assert_eq!(c.count(), 4);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.windows(6);
    assert_eq!(c2.count(), 0);

    let v3: &[i32] = &[];
    let c3 = v3.windows(2);
    assert_eq!(c3.count(), 0);
}

#[test]
fn test_windows_nth() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.windows(2);
    assert_eq!(c.nth(2).unwrap()[1], 3);
    assert_eq!(c.next().unwrap()[0], 3);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.windows(4);
    assert_eq!(c2.nth(1).unwrap()[1], 2);
    assert_eq!(c2.next(), None);
}

#[test]
fn test_windows_nth_back() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let mut c = v.windows(2);
    assert_eq!(c.nth_back(2).unwrap()[0], 2);
    assert_eq!(c.next_back().unwrap()[1], 2);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let mut c2 = v2.windows(4);
    assert_eq!(c2.nth_back(1).unwrap()[1], 1);
    assert_eq!(c2.next_back(), None);
}

#[test]
fn test_windows_last() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5];
    let c = v.windows(2);
    assert_eq!(c.last().unwrap()[1], 5);

    let v2: &[i32] = &[0, 1, 2, 3, 4];
    let c2 = v2.windows(2);
    assert_eq!(c2.last().unwrap()[0], 3);
}

#[test]
fn test_windows_zip() {
    let v1: &[i32] = &[0, 1, 2, 3, 4];
    let v2: &[i32] = &[6, 7, 8, 9, 10];

    let res = v1.windows(2)
        .zip(v2.windows(2))
        .map(|(a, b)| a.iter().sum::<i32>() + b.iter().sum::<i32>())
        .collect::<Vec<_>>();

    assert_eq!(res, [14, 18, 22, 26]);
}

#[test]
#[allow(const_err)]
fn test_iter_ref_consistency() {
    use std::fmt::Debug;

    fn test<T : Copy + Debug + PartialEq>(x : T) {
        let v : &[T] = &[x, x, x];
        let v_ptrs : [*const T; 3] = match v {
            [ref v1, ref v2, ref v3] => [v1 as *const _, v2 as *const _, v3 as *const _],
            _ => unreachable!()
        };
        let len = v.len();

        // nth(i)
        for i in 0..len {
            assert_eq!(&v[i] as *const _, v_ptrs[i]); // check the v_ptrs array, just to be sure
            let nth = v.iter().nth(i).unwrap();
            assert_eq!(nth as *const _, v_ptrs[i]);
        }
        assert_eq!(v.iter().nth(len), None, "nth(len) should return None");

        // stepping through with nth(0)
        {
            let mut it = v.iter();
            for i in 0..len {
                let next = it.nth(0).unwrap();
                assert_eq!(next as *const _, v_ptrs[i]);
            }
            assert_eq!(it.nth(0), None);
        }

        // next()
        {
            let mut it = v.iter();
            for i in 0..len {
                let remaining = len - i;
                assert_eq!(it.size_hint(), (remaining, Some(remaining)));

                let next = it.next().unwrap();
                assert_eq!(next as *const _, v_ptrs[i]);
            }
            assert_eq!(it.size_hint(), (0, Some(0)));
            assert_eq!(it.next(), None, "The final call to next() should return None");
        }

        // next_back()
        {
            let mut it = v.iter();
            for i in 0..len {
                let remaining = len - i;
                assert_eq!(it.size_hint(), (remaining, Some(remaining)));

                let prev = it.next_back().unwrap();
                assert_eq!(prev as *const _, v_ptrs[remaining-1]);
            }
            assert_eq!(it.size_hint(), (0, Some(0)));
            assert_eq!(it.next_back(), None, "The final call to next_back() should return None");
        }
    }

    fn test_mut<T : Copy + Debug + PartialEq>(x : T) {
        let v : &mut [T] = &mut [x, x, x];
        let v_ptrs : [*mut T; 3] = match v {
            [ref v1, ref v2, ref v3] =>
              [v1 as *const _ as *mut _, v2 as *const _ as *mut _, v3 as *const _ as *mut _],
            _ => unreachable!()
        };
        let len = v.len();

        // nth(i)
        for i in 0..len {
            assert_eq!(&mut v[i] as *mut _, v_ptrs[i]); // check the v_ptrs array, just to be sure
            let nth = v.iter_mut().nth(i).unwrap();
            assert_eq!(nth as *mut _, v_ptrs[i]);
        }
        assert_eq!(v.iter().nth(len), None, "nth(len) should return None");

        // stepping through with nth(0)
        {
            let mut it = v.iter();
            for i in 0..len {
                let next = it.nth(0).unwrap();
                assert_eq!(next as *const _, v_ptrs[i]);
            }
            assert_eq!(it.nth(0), None);
        }

        // next()
        {
            let mut it = v.iter_mut();
            for i in 0..len {
                let remaining = len - i;
                assert_eq!(it.size_hint(), (remaining, Some(remaining)));

                let next = it.next().unwrap();
                assert_eq!(next as *mut _, v_ptrs[i]);
            }
            assert_eq!(it.size_hint(), (0, Some(0)));
            assert_eq!(it.next(), None, "The final call to next() should return None");
        }

        // next_back()
        {
            let mut it = v.iter_mut();
            for i in 0..len {
                let remaining = len - i;
                assert_eq!(it.size_hint(), (remaining, Some(remaining)));

                let prev = it.next_back().unwrap();
                assert_eq!(prev as *mut _, v_ptrs[remaining-1]);
            }
            assert_eq!(it.size_hint(), (0, Some(0)));
            assert_eq!(it.next_back(), None, "The final call to next_back() should return None");
        }
    }

    // Make sure iterators and slice patterns yield consistent addresses for various types,
    // including ZSTs.
    test(0u32);
    test(());
    test([0u32; 0]); // ZST with alignment > 0
    test_mut(0u32);
    test_mut(());
    test_mut([0u32; 0]); // ZST with alignment > 0
}

// The current implementation of SliceIndex fails to handle methods
// orthogonally from range types; therefore, it is worth testing
// all of the indexing operations on each input.
mod slice_index {
    // This checks all six indexing methods, given an input range that
    // should succeed. (it is NOT suitable for testing invalid inputs)
    macro_rules! assert_range_eq {
        ($arr:expr, $range:expr, $expected:expr)
        => {
            let mut arr = $arr;
            let mut expected = $expected;
            {
                let s: &[_] = &arr;
                let expected: &[_] = &expected;

                assert_eq!(&s[$range], expected, "(in assertion for: index)");
                assert_eq!(s.get($range), Some(expected), "(in assertion for: get)");
                unsafe {
                    assert_eq!(
                        s.get_unchecked($range), expected,
                        "(in assertion for: get_unchecked)",
                    );
                }
            }
            {
                let s: &mut [_] = &mut arr;
                let expected: &mut [_] = &mut expected;

                assert_eq!(
                    &mut s[$range], expected,
                    "(in assertion for: index_mut)",
                );
                assert_eq!(
                    s.get_mut($range), Some(&mut expected[..]),
                    "(in assertion for: get_mut)",
                );
                unsafe {
                    assert_eq!(
                        s.get_unchecked_mut($range), expected,
                        "(in assertion for: get_unchecked_mut)",
                    );
                }
            }
        }
    }

    // Make sure the macro can actually detect bugs,
    // because if it can't, then what are we even doing here?
    //
    // (Be aware this only demonstrates the ability to detect bugs
    //  in the FIRST method that panics, as the macro is not designed
    //  to be used in `should_panic`)
    #[test]
    #[should_panic(expected = "out of range")]
    fn assert_range_eq_can_fail_by_panic() {
        assert_range_eq!([0, 1, 2], 0..5, [0, 1, 2]);
    }

    // (Be aware this only demonstrates the ability to detect bugs
    //  in the FIRST method it calls, as the macro is not designed
    //  to be used in `should_panic`)
    #[test]
    #[should_panic(expected = "==")]
    fn assert_range_eq_can_fail_by_inequality() {
        assert_range_eq!([0, 1, 2], 0..2, [0, 1, 2]);
    }

    // Test cases for bad index operations.
    //
    // This generates `should_panic` test cases for Index/IndexMut
    // and `None` test cases for get/get_mut.
    macro_rules! panic_cases {
        ($(
            // each test case needs a unique name to namespace the tests
            in mod $case_name:ident {
                data: $data:expr;

                // optional:
                //
                // one or more similar inputs for which data[input] succeeds,
                // and the corresponding output as an array.  This helps validate
                // "critical points" where an input range straddles the boundary
                // between valid and invalid.
                // (such as the input `len..len`, which is just barely valid)
                $(
                    good: data[$good:expr] == $output:expr;
                )*

                bad: data[$bad:expr];
                message: $expect_msg:expr;
            }
        )*) => {$(
            mod $case_name {
                #[test]
                fn pass() {
                    let mut v = $data;

                    $( assert_range_eq!($data, $good, $output); )*

                    {
                        let v: &[_] = &v;
                        assert_eq!(v.get($bad), None, "(in None assertion for get)");
                    }

                    {
                        let v: &mut [_] = &mut v;
                        assert_eq!(v.get_mut($bad), None, "(in None assertion for get_mut)");
                    }
                }

                #[test]
                #[should_panic(expected = $expect_msg)]
                fn index_fail() {
                    let v = $data;
                    let v: &[_] = &v;
                    let _v = &v[$bad];
                }

                #[test]
                #[should_panic(expected = $expect_msg)]
                fn index_mut_fail() {
                    let mut v = $data;
                    let v: &mut [_] = &mut v;
                    let _v = &mut v[$bad];
                }
            }
        )*};
    }

    #[test]
    fn simple() {
        let v = [0, 1, 2, 3, 4, 5];

        assert_range_eq!(v, .., [0, 1, 2, 3, 4, 5]);
        assert_range_eq!(v, ..2, [0, 1]);
        assert_range_eq!(v, ..=1, [0, 1]);
        assert_range_eq!(v, 2.., [2, 3, 4, 5]);
        assert_range_eq!(v, 1..4, [1, 2, 3]);
        assert_range_eq!(v, 1..=3, [1, 2, 3]);
    }

    panic_cases! {
        in mod rangefrom_len {
            data: [0, 1, 2, 3, 4, 5];

            good: data[6..] == [];
            bad: data[7..];
            message: "but ends at"; // perhaps not ideal
        }

        in mod rangeto_len {
            data: [0, 1, 2, 3, 4, 5];

            good: data[..6] == [0, 1, 2, 3, 4, 5];
            bad: data[..7];
            message: "out of range";
        }

        in mod rangetoinclusive_len {
            data: [0, 1, 2, 3, 4, 5];

            good: data[..=5] == [0, 1, 2, 3, 4, 5];
            bad: data[..=6];
            message: "out of range";
        }

        in mod range_len_len {
            data: [0, 1, 2, 3, 4, 5];

            good: data[6..6] == [];
            bad: data[7..7];
            message: "out of range";
        }

        in mod rangeinclusive_len_len {
            data: [0, 1, 2, 3, 4, 5];

            good: data[6..=5] == [];
            bad: data[7..=6];
            message: "out of range";
        }
    }

    panic_cases! {
        in mod range_neg_width {
            data: [0, 1, 2, 3, 4, 5];

            good: data[4..4] == [];
            bad: data[4..3];
            message: "but ends at";
        }

        in mod rangeinclusive_neg_width {
            data: [0, 1, 2, 3, 4, 5];

            good: data[4..=3] == [];
            bad: data[4..=2];
            message: "but ends at";
        }
    }

    panic_cases! {
        in mod rangeinclusive_overflow {
            data: [0, 1];

            // note: using 0 specifically ensures that the result of overflowing is 0..0,
            //       so that `get` doesn't simply return None for the wrong reason.
            bad: data[0 ..= ::std::usize::MAX];
            message: "maximum usize";
        }

        in mod rangetoinclusive_overflow {
            data: [0, 1];

            bad: data[..= ::std::usize::MAX];
            message: "maximum usize";
        }
    } // panic_cases!
}

#[test]
fn test_find_rfind() {
    let v = [0, 1, 2, 3, 4, 5];
    let mut iter = v.iter();
    let mut i = v.len();
    while let Some(&elt) = iter.rfind(|_| true) {
        i -= 1;
        assert_eq!(elt, v[i]);
    }
    assert_eq!(i, 0);
    assert_eq!(v.iter().rfind(|&&x| x <= 3), Some(&3));
}

#[test]
fn test_iter_folds() {
    let a = [1, 2, 3, 4, 5]; // len>4 so the unroll is used
    assert_eq!(a.iter().fold(0, |acc, &x| 2*acc + x), 57);
    assert_eq!(a.iter().rfold(0, |acc, &x| 2*acc + x), 129);
    let fold = |acc: i32, &x| acc.checked_mul(2)?.checked_add(x);
    assert_eq!(a.iter().try_fold(0, &fold), Some(57));
    assert_eq!(a.iter().try_rfold(0, &fold), Some(129));

    // short-circuiting try_fold, through other methods
    let a = [0, 1, 2, 3, 5, 5, 5, 7, 8, 9];
    let mut iter = a.iter();
    assert_eq!(iter.position(|&x| x == 3), Some(3));
    assert_eq!(iter.rfind(|&&x| x == 5), Some(&5));
    assert_eq!(iter.len(), 2);
}

#[test]
fn test_rotate_left() {
    const N: usize = 600;
    let a: &mut [_] = &mut [0; N];
    for i in 0..N {
        a[i] = i;
    }

    a.rotate_left(42);
    let k = N - 42;

    for i in 0..N {
        assert_eq!(a[(i + k) % N], i);
    }
}

#[test]
fn test_rotate_right() {
    const N: usize = 600;
    let a: &mut [_] = &mut [0; N];
    for i in 0..N {
        a[i] = i;
    }

    a.rotate_right(42);

    for i in 0..N {
        assert_eq!(a[(i + 42) % N], i);
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn sort_unstable() {
    use core::cmp::Ordering::{Equal, Greater, Less};
    use core::slice::heapsort;
    use rand::{FromEntropy, Rng, rngs::SmallRng, seq::SliceRandom};

    #[cfg(not(miri))] // Miri is too slow
    let large_range = 500..510;
    #[cfg(not(miri))] // Miri is too slow
    let rounds = 100;

    #[cfg(miri)]
    let large_range = 0..0; // empty range
    #[cfg(miri)]
    let rounds = 1;

    let mut v = [0; 600];
    let mut tmp = [0; 600];
    let mut rng = SmallRng::from_entropy();

    for len in (2..25).chain(large_range) {
        let v = &mut v[0..len];
        let tmp = &mut tmp[0..len];

        for &modulus in &[5, 10, 100, 1000] {
            for _ in 0..rounds {
                for i in 0..len {
                    v[i] = rng.gen::<i32>() % modulus;
                }

                // Sort in default order.
                tmp.copy_from_slice(v);
                tmp.sort_unstable();
                assert!(tmp.windows(2).all(|w| w[0] <= w[1]));

                // Sort in ascending order.
                tmp.copy_from_slice(v);
                tmp.sort_unstable_by(|a, b| a.cmp(b));
                assert!(tmp.windows(2).all(|w| w[0] <= w[1]));

                // Sort in descending order.
                tmp.copy_from_slice(v);
                tmp.sort_unstable_by(|a, b| b.cmp(a));
                assert!(tmp.windows(2).all(|w| w[0] >= w[1]));

                // Test heapsort using `<` operator.
                tmp.copy_from_slice(v);
                heapsort(tmp, |a, b| a < b);
                assert!(tmp.windows(2).all(|w| w[0] <= w[1]));

                // Test heapsort using `>` operator.
                tmp.copy_from_slice(v);
                heapsort(tmp, |a, b| a > b);
                assert!(tmp.windows(2).all(|w| w[0] >= w[1]));
            }
        }
    }

    // Sort using a completely random comparison function.
    // This will reorder the elements *somehow*, but won't panic.
    for i in 0..v.len() {
        v[i] = i as i32;
    }
    v.sort_unstable_by(|_, _| *[Less, Equal, Greater].choose(&mut rng).unwrap());
    v.sort_unstable();
    for i in 0..v.len() {
        assert_eq!(v[i], i as i32);
    }

    // Should not panic.
    [0i32; 0].sort_unstable();
    [(); 10].sort_unstable();
    [(); 100].sort_unstable();

    let mut v = [0xDEADBEEFu64];
    v.sort_unstable();
    assert!(v == [0xDEADBEEF]);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(miri))] // Miri is too slow
fn partition_at_index() {
    use core::cmp::Ordering::{Equal, Greater, Less};
    use rand::rngs::SmallRng;
    use rand::seq::SliceRandom;
    use rand::{FromEntropy, Rng};

    let mut rng = SmallRng::from_entropy();

    for len in (2..21).chain(500..501) {
        let mut orig = vec![0; len];

        for &modulus in &[5, 10, 1000] {
            for _ in 0..10 {
                for i in 0..len {
                    orig[i] = rng.gen::<i32>() % modulus;
                }

                let v_sorted = {
                    let mut v = orig.clone();
                    v.sort();
                    v
                };

                // Sort in default order.
                for pivot in 0..len {
                    let mut v = orig.clone();
                    v.partition_at_index(pivot);

                    assert_eq!(v_sorted[pivot], v[pivot]);
                    for i in 0..pivot {
                        for j in pivot..len {
                            assert!(v[i] <= v[j]);
                        }
                    }
                }

                // Sort in ascending order.
                for pivot in 0..len {
                    let mut v = orig.clone();
                    let (left, pivot, right) = v.partition_at_index_by(pivot, |a, b| a.cmp(b));

                    assert_eq!(left.len() + right.len(), len - 1);

                    for l in left {
                        assert!(l <= pivot);
                        for r in right.iter_mut() {
                            assert!(l <= r);
                            assert!(pivot <= r);
                        }
                    }
                }

                // Sort in descending order.
                let sort_descending_comparator = |a: &i32, b: &i32| b.cmp(a);
                let v_sorted_descending = {
                    let mut v = orig.clone();
                    v.sort_by(sort_descending_comparator);
                    v
                };

                for pivot in 0..len {
                    let mut v = orig.clone();
                    v.partition_at_index_by(pivot, sort_descending_comparator);

                    assert_eq!(v_sorted_descending[pivot], v[pivot]);
                    for i in 0..pivot {
                        for j in pivot..len {
                            assert!(v[j] <= v[i]);
                        }
                    }
                }
            }
        }
    }

    // Sort at index using a completely random comparison function.
    // This will reorder the elements *somehow*, but won't panic.
    let mut v = [0; 500];
    for i in 0..v.len() {
        v[i] = i as i32;
    }

    for pivot in 0..v.len() {
        v.partition_at_index_by(pivot, |_, _| *[Less, Equal, Greater].choose(&mut rng).unwrap());
        v.sort();
        for i in 0..v.len() {
            assert_eq!(v[i], i as i32);
        }
    }

    // Should not panic.
    [(); 10].partition_at_index(0);
    [(); 10].partition_at_index(5);
    [(); 10].partition_at_index(9);
    [(); 100].partition_at_index(0);
    [(); 100].partition_at_index(50);
    [(); 100].partition_at_index(99);

    let mut v = [0xDEADBEEFu64];
    v.partition_at_index(0);
    assert!(v == [0xDEADBEEF]);
}

#[test]
#[should_panic(expected = "index 0 greater than length of slice")]
fn partition_at_index_zero_length() {
    [0i32; 0].partition_at_index(0);
}

#[test]
#[should_panic(expected = "index 20 greater than length of slice")]
fn partition_at_index_past_length() {
    [0i32; 10].partition_at_index(20);
}

pub mod memchr {
    use core::slice::memchr::{memchr, memrchr};

    // test fallback implementations on all platforms
    #[test]
    fn matches_one() {
        assert_eq!(Some(0), memchr(b'a', b"a"));
    }

    #[test]
    fn matches_begin() {
        assert_eq!(Some(0), memchr(b'a', b"aaaa"));
    }

    #[test]
    fn matches_end() {
        assert_eq!(Some(4), memchr(b'z', b"aaaaz"));
    }

    #[test]
    fn matches_nul() {
        assert_eq!(Some(4), memchr(b'\x00', b"aaaa\x00"));
    }

    #[test]
    fn matches_past_nul() {
        assert_eq!(Some(5), memchr(b'z', b"aaaa\x00z"));
    }

    #[test]
    fn no_match_empty() {
        assert_eq!(None, memchr(b'a', b""));
    }

    #[test]
    fn no_match() {
        assert_eq!(None, memchr(b'a', b"xyz"));
    }

    #[test]
    fn matches_one_reversed() {
        assert_eq!(Some(0), memrchr(b'a', b"a"));
    }

    #[test]
    fn matches_begin_reversed() {
        assert_eq!(Some(3), memrchr(b'a', b"aaaa"));
    }

    #[test]
    fn matches_end_reversed() {
        assert_eq!(Some(0), memrchr(b'z', b"zaaaa"));
    }

    #[test]
    fn matches_nul_reversed() {
        assert_eq!(Some(4), memrchr(b'\x00', b"aaaa\x00"));
    }

    #[test]
    fn matches_past_nul_reversed() {
        assert_eq!(Some(0), memrchr(b'z', b"z\x00aaaa"));
    }

    #[test]
    fn no_match_empty_reversed() {
        assert_eq!(None, memrchr(b'a', b""));
    }

    #[test]
    fn no_match_reversed() {
        assert_eq!(None, memrchr(b'a', b"xyz"));
    }

    #[test]
    fn each_alignment_reversed() {
        let mut data = [1u8; 64];
        let needle = 2;
        let pos = 40;
        data[pos] = needle;
        for start in 0..16 {
            assert_eq!(Some(pos - start), memrchr(needle, &data[start..]));
        }
    }
}

#[test]
#[cfg(not(miri))] // Miri cannot compute actual alignment of an allocation
fn test_align_to_simple() {
    let bytes = [1u8, 2, 3, 4, 5, 6, 7];
    let (prefix, aligned, suffix) = unsafe { bytes.align_to::<u16>() };
    assert_eq!(aligned.len(), 3);
    assert!(prefix == [1] || suffix == [7]);
    let expect1 = [1 << 8 | 2, 3 << 8 | 4, 5 << 8 | 6];
    let expect2 = [1 | 2 << 8, 3 | 4 << 8, 5 | 6 << 8];
    let expect3 = [2 << 8 | 3, 4 << 8 | 5, 6 << 8 | 7];
    let expect4 = [2 | 3 << 8, 4 | 5 << 8, 6 | 7 << 8];
    assert!(aligned == expect1 || aligned == expect2 || aligned == expect3 || aligned == expect4,
            "aligned={:?} expected={:?} || {:?} || {:?} || {:?}",
            aligned, expect1, expect2, expect3, expect4);
}

#[test]
fn test_align_to_zst() {
    let bytes = [1, 2, 3, 4, 5, 6, 7];
    let (prefix, aligned, suffix) = unsafe { bytes.align_to::<()>() };
    assert_eq!(aligned.len(), 0);
    assert!(prefix == [1, 2, 3, 4, 5, 6, 7] || suffix == [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
#[cfg(not(miri))] // Miri cannot compute actual alignment of an allocation
fn test_align_to_non_trivial() {
    #[repr(align(8))] struct U64(u64, u64);
    #[repr(align(8))] struct U64U64U32(u64, u64, u32);
    let data = [U64(1, 2), U64(3, 4), U64(5, 6), U64(7, 8), U64(9, 10), U64(11, 12), U64(13, 14),
                U64(15, 16)];
    let (prefix, aligned, suffix) = unsafe { data.align_to::<U64U64U32>() };
    assert_eq!(aligned.len(), 4);
    assert_eq!(prefix.len() + suffix.len(), 2);
}

#[test]
fn test_align_to_empty_mid() {
    use core::mem;

    // Make sure that we do not create empty unaligned slices for the mid part, even when the
    // overall slice is too short to contain an aligned address.
    let bytes = [1, 2, 3, 4, 5, 6, 7];
    type Chunk = u32;
    for offset in 0..4 {
        let (_, mid, _) = unsafe { bytes[offset..offset+1].align_to::<Chunk>() };
        assert_eq!(mid.as_ptr() as usize % mem::align_of::<Chunk>(), 0);
    }
}

#[test]
fn test_slice_partition_dedup_by() {
    let mut slice: [i32; 9] = [1, -1, 2, 3, 1, -5, 5, -2, 2];

    let (dedup, duplicates) = slice.partition_dedup_by(|a, b| a.abs() == b.abs());

    assert_eq!(dedup, [1, 2, 3, 1, -5, -2]);
    assert_eq!(duplicates, [5, -1, 2]);
}

#[test]
fn test_slice_partition_dedup_empty() {
    let mut slice: [i32; 0] = [];

    let (dedup, duplicates) = slice.partition_dedup();

    assert_eq!(dedup, []);
    assert_eq!(duplicates, []);
}

#[test]
fn test_slice_partition_dedup_one() {
    let mut slice = [12];

    let (dedup, duplicates) = slice.partition_dedup();

    assert_eq!(dedup, [12]);
    assert_eq!(duplicates, []);
}

#[test]
fn test_slice_partition_dedup_multiple_ident() {
    let mut slice = [12, 12, 12, 12, 12, 11, 11, 11, 11, 11, 11];

    let (dedup, duplicates) = slice.partition_dedup();

    assert_eq!(dedup, [12, 11]);
    assert_eq!(duplicates, [12, 12, 12, 12, 11, 11, 11, 11, 11]);
}

#[test]
fn test_slice_partition_dedup_partialeq() {
    #[derive(Debug)]
    struct Foo(i32, i32);

    impl PartialEq for Foo {
        fn eq(&self, other: &Foo) -> bool {
            self.0 == other.0
        }
    }

    let mut slice = [Foo(0, 1), Foo(0, 5), Foo(1, 7), Foo(1, 9)];

    let (dedup, duplicates) = slice.partition_dedup();

    assert_eq!(dedup, [Foo(0, 1), Foo(1, 7)]);
    assert_eq!(duplicates, [Foo(0, 5), Foo(1, 9)]);
}

#[test]
fn test_copy_within() {
    // Start to end, with a RangeTo.
    let mut bytes = *b"Hello, World!";
    bytes.copy_within(..3, 10);
    assert_eq!(&bytes, b"Hello, WorHel");

    // End to start, with a RangeFrom.
    let mut bytes = *b"Hello, World!";
    bytes.copy_within(10.., 0);
    assert_eq!(&bytes, b"ld!lo, World!");

    // Overlapping, with a RangeInclusive.
    let mut bytes = *b"Hello, World!";
    bytes.copy_within(0..=11, 1);
    assert_eq!(&bytes, b"HHello, World");

    // Whole slice, with a RangeFull.
    let mut bytes = *b"Hello, World!";
    bytes.copy_within(.., 0);
    assert_eq!(&bytes, b"Hello, World!");

    // Ensure that copying at the end of slice won't cause UB.
    let mut bytes = *b"Hello, World!";
    bytes.copy_within(13..13, 5);
    assert_eq!(&bytes, b"Hello, World!");
    bytes.copy_within(5..5, 13);
    assert_eq!(&bytes, b"Hello, World!");
}

#[test]
#[should_panic(expected = "src is out of bounds")]
fn test_copy_within_panics_src_too_long() {
    let mut bytes = *b"Hello, World!";
    // The length is only 13, so 14 is out of bounds.
    bytes.copy_within(10..14, 0);
}

#[test]
#[should_panic(expected = "dest is out of bounds")]
fn test_copy_within_panics_dest_too_long() {
    let mut bytes = *b"Hello, World!";
    // The length is only 13, so a slice of length 4 starting at index 10 is out of bounds.
    bytes.copy_within(0..4, 10);
}
#[test]
#[should_panic(expected = "src end is before src start")]
fn test_copy_within_panics_src_inverted() {
    let mut bytes = *b"Hello, World!";
    // 2 is greater than 1, so this range is invalid.
    bytes.copy_within(2..1, 0);
}
#[test]
#[should_panic(expected = "attempted to index slice up to maximum usize")]
fn test_copy_within_panics_src_out_of_bounds() {
    let mut bytes = *b"Hello, World!";
    // 2 is greater than 1, so this range is invalid.
    bytes.copy_within(usize::max_value()..=usize::max_value(), 0);
}

#[test]
fn test_is_sorted() {
    let empty: [i32; 0] = [];

    assert!([1, 2, 2, 9].is_sorted());
    assert!(![1, 3, 2].is_sorted());
    assert!([0].is_sorted());
    assert!(empty.is_sorted());
    assert!(![0.0, 1.0, std::f32::NAN].is_sorted());
    assert!([-2, -1, 0, 3].is_sorted());
    assert!(![-2i32, -1, 0, 3].is_sorted_by_key(|n| n.abs()));
    assert!(!["c", "bb", "aaa"].is_sorted());
    assert!(["c", "bb", "aaa"].is_sorted_by_key(|s| s.len()));
}
