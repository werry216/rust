// NOTE: The following code was generated by "src/libcore/unicode/printable.py",
//       do not edit directly!

fn check(x: u16, singletonuppers: &[(u8, u8)], singletonlowers: &[u8],
         normal: &[u8]) -> bool {
    let xupper = (x >> 8) as u8;
    let mut lowerstart = 0;
    for &(upper, lowercount) in singletonuppers {
        let lowerend = lowerstart + lowercount as usize;
        if xupper == upper {
            for &lower in &singletonlowers[lowerstart..lowerend] {
                if lower == x as u8 {
                    return false;
                }
            }
        } else if xupper < upper {
            break;
        }
        lowerstart = lowerend;
    }

    let mut x = x as i32;
    let mut normal = normal.iter().cloned();
    let mut current = true;
    while let Some(v) = normal.next() {
        let len = if v & 0x80 != 0 {
            ((v & 0x7f) as i32) << 8 | normal.next().unwrap() as i32
        } else {
            v as i32
        };
        x -= len;
        if x < 0 {
            break;
        }
        current = !current;
    }
    current
}

pub(crate) fn is_printable(x: char) -> bool {
    let x = x as u32;
    let lower = x as u16;
    if x < 0x10000 {
        check(lower, SINGLETONS0U, SINGLETONS0L, NORMAL0)
    } else if x < 0x20000 {
        check(lower, SINGLETONS1U, SINGLETONS1L, NORMAL1)
    } else {
        if 0x2a6d7 <= x && x < 0x2a700 {
            return false;
        }
        if 0x2b735 <= x && x < 0x2b740 {
            return false;
        }
        if 0x2b81e <= x && x < 0x2b820 {
            return false;
        }
        if 0x2cea2 <= x && x < 0x2ceb0 {
            return false;
        }
        if 0x2ebe1 <= x && x < 0x2f800 {
            return false;
        }
        if 0x2fa1e <= x && x < 0xe0100 {
            return false;
        }
        if 0xe01f0 <= x && x < 0x110000 {
            return false;
        }
        true
    }
}

const SINGLETONS0U: &[(u8, u8)] = &[
    (0x00, 1),
    (0x03, 5),
    (0x05, 6),
    (0x06, 3),
    (0x07, 6),
    (0x08, 8),
    (0x09, 17),
    (0x0a, 28),
    (0x0b, 25),
    (0x0c, 20),
    (0x0d, 18),
    (0x0e, 13),
    (0x0f, 4),
    (0x10, 3),
    (0x12, 18),
    (0x13, 9),
    (0x16, 1),
    (0x17, 5),
    (0x18, 2),
    (0x19, 3),
    (0x1a, 7),
    (0x1c, 2),
    (0x1d, 1),
    (0x1f, 22),
    (0x20, 3),
    (0x2b, 4),
    (0x2c, 2),
    (0x2d, 11),
    (0x2e, 1),
    (0x30, 3),
    (0x31, 2),
    (0x32, 1),
    (0xa7, 2),
    (0xa9, 2),
    (0xaa, 4),
    (0xab, 8),
    (0xfa, 2),
    (0xfb, 5),
    (0xfd, 4),
    (0xfe, 3),
    (0xff, 9),
];
const SINGLETONS0L: &[u8] = &[
    0xad, 0x78, 0x79, 0x8b, 0x8d, 0xa2, 0x30, 0x57,
    0x58, 0x8b, 0x8c, 0x90, 0x1c, 0x1d, 0xdd, 0x0e,
    0x0f, 0x4b, 0x4c, 0xfb, 0xfc, 0x2e, 0x2f, 0x3f,
    0x5c, 0x5d, 0x5f, 0xb5, 0xe2, 0x84, 0x8d, 0x8e,
    0x91, 0x92, 0xa9, 0xb1, 0xba, 0xbb, 0xc5, 0xc6,
    0xc9, 0xca, 0xde, 0xe4, 0xe5, 0xff, 0x00, 0x04,
    0x11, 0x12, 0x29, 0x31, 0x34, 0x37, 0x3a, 0x3b,
    0x3d, 0x49, 0x4a, 0x5d, 0x84, 0x8e, 0x92, 0xa9,
    0xb1, 0xb4, 0xba, 0xbb, 0xc6, 0xca, 0xce, 0xcf,
    0xe4, 0xe5, 0x00, 0x04, 0x0d, 0x0e, 0x11, 0x12,
    0x29, 0x31, 0x34, 0x3a, 0x3b, 0x45, 0x46, 0x49,
    0x4a, 0x5e, 0x64, 0x65, 0x84, 0x91, 0x9b, 0x9d,
    0xc9, 0xce, 0xcf, 0x0d, 0x11, 0x29, 0x45, 0x49,
    0x57, 0x64, 0x65, 0x8d, 0x91, 0xa9, 0xb4, 0xba,
    0xbb, 0xc5, 0xc9, 0xdf, 0xe4, 0xe5, 0xf0, 0x04,
    0x0d, 0x11, 0x45, 0x49, 0x64, 0x65, 0x80, 0x81,
    0x84, 0xb2, 0xbc, 0xbe, 0xbf, 0xd5, 0xd7, 0xf0,
    0xf1, 0x83, 0x85, 0x8b, 0xa4, 0xa6, 0xbe, 0xbf,
    0xc5, 0xc7, 0xce, 0xcf, 0xda, 0xdb, 0x48, 0x98,
    0xbd, 0xcd, 0xc6, 0xce, 0xcf, 0x49, 0x4e, 0x4f,
    0x57, 0x59, 0x5e, 0x5f, 0x89, 0x8e, 0x8f, 0xb1,
    0xb6, 0xb7, 0xbf, 0xc1, 0xc6, 0xc7, 0xd7, 0x11,
    0x16, 0x17, 0x5b, 0x5c, 0xf6, 0xf7, 0xfe, 0xff,
    0x80, 0x0d, 0x6d, 0x71, 0xde, 0xdf, 0x0e, 0x0f,
    0x1f, 0x6e, 0x6f, 0x1c, 0x1d, 0x5f, 0x7d, 0x7e,
    0xae, 0xaf, 0xbb, 0xbc, 0xfa, 0x16, 0x17, 0x1e,
    0x1f, 0x46, 0x47, 0x4e, 0x4f, 0x58, 0x5a, 0x5c,
    0x5e, 0x7e, 0x7f, 0xb5, 0xc5, 0xd4, 0xd5, 0xdc,
    0xf0, 0xf1, 0xf5, 0x72, 0x73, 0x8f, 0x74, 0x75,
    0x96, 0x97, 0x2f, 0x5f, 0x26, 0x2e, 0x2f, 0xa7,
    0xaf, 0xb7, 0xbf, 0xc7, 0xcf, 0xd7, 0xdf, 0x9a,
    0x40, 0x97, 0x98, 0x30, 0x8f, 0x1f, 0xc0, 0xc1,
    0xce, 0xff, 0x4e, 0x4f, 0x5a, 0x5b, 0x07, 0x08,
    0x0f, 0x10, 0x27, 0x2f, 0xee, 0xef, 0x6e, 0x6f,
    0x37, 0x3d, 0x3f, 0x42, 0x45, 0x90, 0x91, 0xfe,
    0xff, 0x53, 0x67, 0x75, 0xc8, 0xc9, 0xd0, 0xd1,
    0xd8, 0xd9, 0xe7, 0xfe, 0xff,
];
const SINGLETONS1U: &[(u8, u8)] = &[
    (0x00, 6),
    (0x01, 1),
    (0x03, 1),
    (0x04, 2),
    (0x08, 8),
    (0x09, 2),
    (0x0a, 5),
    (0x0b, 2),
    (0x10, 1),
    (0x11, 4),
    (0x12, 5),
    (0x13, 17),
    (0x14, 2),
    (0x15, 2),
    (0x17, 2),
    (0x19, 4),
    (0x1c, 5),
    (0x1d, 8),
    (0x24, 1),
    (0x6a, 3),
    (0x6b, 2),
    (0xbc, 2),
    (0xd1, 2),
    (0xd4, 12),
    (0xd5, 9),
    (0xd6, 2),
    (0xd7, 2),
    (0xda, 1),
    (0xe0, 5),
    (0xe1, 2),
    (0xe8, 2),
    (0xee, 32),
    (0xf0, 4),
    (0xf9, 6),
    (0xfa, 2),
];
const SINGLETONS1L: &[u8] = &[
    0x0c, 0x27, 0x3b, 0x3e, 0x4e, 0x4f, 0x8f, 0x9e,
    0x9e, 0x9f, 0x06, 0x07, 0x09, 0x36, 0x3d, 0x3e,
    0x56, 0xf3, 0xd0, 0xd1, 0x04, 0x14, 0x18, 0x36,
    0x37, 0x56, 0x57, 0xbd, 0x35, 0xce, 0xcf, 0xe0,
    0x12, 0x87, 0x89, 0x8e, 0x9e, 0x04, 0x0d, 0x0e,
    0x11, 0x12, 0x29, 0x31, 0x34, 0x3a, 0x45, 0x46,
    0x49, 0x4a, 0x4e, 0x4f, 0x64, 0x65, 0x5a, 0x5c,
    0xb6, 0xb7, 0x1b, 0x1c, 0xa8, 0xa9, 0xd8, 0xd9,
    0x09, 0x37, 0x90, 0x91, 0xa8, 0x07, 0x0a, 0x3b,
    0x3e, 0x66, 0x69, 0x8f, 0x92, 0x6f, 0x5f, 0xee,
    0xef, 0x5a, 0x62, 0x9a, 0x9b, 0x27, 0x28, 0x55,
    0x9d, 0xa0, 0xa1, 0xa3, 0xa4, 0xa7, 0xa8, 0xad,
    0xba, 0xbc, 0xc4, 0x06, 0x0b, 0x0c, 0x15, 0x1d,
    0x3a, 0x3f, 0x45, 0x51, 0xa6, 0xa7, 0xcc, 0xcd,
    0xa0, 0x07, 0x19, 0x1a, 0x22, 0x25, 0x3e, 0x3f,
    0xc5, 0xc6, 0x04, 0x20, 0x23, 0x25, 0x26, 0x28,
    0x33, 0x38, 0x3a, 0x48, 0x4a, 0x4c, 0x50, 0x53,
    0x55, 0x56, 0x58, 0x5a, 0x5c, 0x5e, 0x60, 0x63,
    0x65, 0x66, 0x6b, 0x73, 0x78, 0x7d, 0x7f, 0x8a,
    0xa4, 0xaa, 0xaf, 0xb0, 0xc0, 0xd0, 0x0c, 0x72,
    0xa3, 0xa4, 0xcb, 0xcc, 0x6e, 0x6f,
];
const NORMAL0: &[u8] = &[
    0x00, 0x20,
    0x5f, 0x22,
    0x82, 0xdf, 0x04,
    0x82, 0x44, 0x08,
    0x1b, 0x04,
    0x06, 0x11,
    0x81, 0xac, 0x0e,
    0x80, 0xab, 0x35,
    0x1e, 0x15,
    0x80, 0xe0, 0x03,
    0x19, 0x08,
    0x01, 0x04,
    0x2f, 0x04,
    0x34, 0x04,
    0x07, 0x03,
    0x01, 0x07,
    0x06, 0x07,
    0x11, 0x0a,
    0x50, 0x0f,
    0x12, 0x07,
    0x55, 0x08,
    0x02, 0x04,
    0x1c, 0x0a,
    0x09, 0x03,
    0x08, 0x03,
    0x07, 0x03,
    0x02, 0x03,
    0x03, 0x03,
    0x0c, 0x04,
    0x05, 0x03,
    0x0b, 0x06,
    0x01, 0x0e,
    0x15, 0x05,
    0x3a, 0x03,
    0x11, 0x07,
    0x06, 0x05,
    0x10, 0x07,
    0x57, 0x07,
    0x02, 0x07,
    0x15, 0x0d,
    0x50, 0x04,
    0x43, 0x03,
    0x2d, 0x03,
    0x01, 0x04,
    0x11, 0x06,
    0x0f, 0x0c,
    0x3a, 0x04,
    0x1d, 0x25,
    0x5f, 0x20,
    0x6d, 0x04,
    0x6a, 0x25,
    0x80, 0xc8, 0x05,
    0x82, 0xb0, 0x03,
    0x1a, 0x06,
    0x82, 0xfd, 0x03,
    0x59, 0x07,
    0x15, 0x0b,
    0x17, 0x09,
    0x14, 0x0c,
    0x14, 0x0c,
    0x6a, 0x06,
    0x0a, 0x06,
    0x1a, 0x06,
    0x59, 0x07,
    0x2b, 0x05,
    0x46, 0x0a,
    0x2c, 0x04,
    0x0c, 0x04,
    0x01, 0x03,
    0x31, 0x0b,
    0x2c, 0x04,
    0x1a, 0x06,
    0x0b, 0x03,
    0x80, 0xac, 0x06,
    0x0a, 0x06,
    0x1f, 0x41,
    0x4c, 0x04,
    0x2d, 0x03,
    0x74, 0x08,
    0x3c, 0x03,
    0x0f, 0x03,
    0x3c, 0x07,
    0x38, 0x08,
    0x2b, 0x05,
    0x82, 0xff, 0x11,
    0x18, 0x08,
    0x2f, 0x11,
    0x2d, 0x03,
    0x20, 0x10,
    0x21, 0x0f,
    0x80, 0x8c, 0x04,
    0x82, 0x97, 0x19,
    0x0b, 0x15,
    0x88, 0x94, 0x05,
    0x2f, 0x05,
    0x3b, 0x07,
    0x02, 0x0e,
    0x18, 0x09,
    0x80, 0xb0, 0x30,
    0x74, 0x0c,
    0x80, 0xd6, 0x1a,
    0x0c, 0x05,
    0x80, 0xff, 0x05,
    0x80, 0xb6, 0x05,
    0x24, 0x0c,
    0x9b, 0xc6, 0x0a,
    0xd2, 0x30, 0x10,
    0x84, 0x8d, 0x03,
    0x37, 0x09,
    0x81, 0x5c, 0x14,
    0x80, 0xb8, 0x08,
    0x80, 0xc7, 0x30,
    0x35, 0x04,
    0x0a, 0x06,
    0x38, 0x08,
    0x46, 0x08,
    0x0c, 0x06,
    0x74, 0x0b,
    0x1e, 0x03,
    0x5a, 0x04,
    0x59, 0x09,
    0x80, 0x83, 0x18,
    0x1c, 0x0a,
    0x16, 0x09,
    0x48, 0x08,
    0x80, 0x8a, 0x06,
    0xab, 0xa4, 0x0c,
    0x17, 0x04,
    0x31, 0xa1, 0x04,
    0x81, 0xda, 0x26,
    0x07, 0x0c,
    0x05, 0x05,
    0x80, 0xa5, 0x11,
    0x81, 0x6d, 0x10,
    0x78, 0x28,
    0x2a, 0x06,
    0x4c, 0x04,
    0x80, 0x8d, 0x04,
    0x80, 0xbe, 0x03,
    0x1b, 0x03,
    0x0f, 0x0d,
];
const NORMAL1: &[u8] = &[
    0x5e, 0x22,
    0x7b, 0x05,
    0x03, 0x04,
    0x2d, 0x03,
    0x65, 0x04,
    0x01, 0x2f,
    0x2e, 0x80, 0x82,
    0x1d, 0x03,
    0x31, 0x0f,
    0x1c, 0x04,
    0x24, 0x09,
    0x1e, 0x05,
    0x2b, 0x05,
    0x44, 0x04,
    0x0e, 0x2a,
    0x80, 0xaa, 0x06,
    0x24, 0x04,
    0x24, 0x04,
    0x28, 0x08,
    0x34, 0x0b,
    0x01, 0x80, 0x90,
    0x81, 0x37, 0x09,
    0x16, 0x0a,
    0x08, 0x80, 0x98,
    0x39, 0x03,
    0x63, 0x08,
    0x09, 0x30,
    0x16, 0x05,
    0x21, 0x03,
    0x1b, 0x05,
    0x01, 0x40,
    0x38, 0x04,
    0x4b, 0x05,
    0x2f, 0x04,
    0x0a, 0x07,
    0x09, 0x07,
    0x40, 0x20,
    0x27, 0x04,
    0x0c, 0x09,
    0x36, 0x03,
    0x3a, 0x05,
    0x1a, 0x07,
    0x04, 0x0c,
    0x07, 0x50,
    0x49, 0x37,
    0x33, 0x0d,
    0x33, 0x07,
    0x2e, 0x08,
    0x0a, 0x81, 0x26,
    0x1f, 0x80, 0x81,
    0x28, 0x08,
    0x2a, 0x80, 0x86,
    0x17, 0x09,
    0x4e, 0x04,
    0x1e, 0x0f,
    0x43, 0x0e,
    0x19, 0x07,
    0x0a, 0x06,
    0x47, 0x09,
    0x27, 0x09,
    0x75, 0x0b,
    0x3f, 0x41,
    0x2a, 0x06,
    0x3b, 0x05,
    0x0a, 0x06,
    0x51, 0x06,
    0x01, 0x05,
    0x10, 0x03,
    0x05, 0x80, 0x8b,
    0x60, 0x20,
    0x48, 0x08,
    0x0a, 0x80, 0xa6,
    0x5e, 0x22,
    0x45, 0x0b,
    0x0a, 0x06,
    0x0d, 0x13,
    0x39, 0x07,
    0x0a, 0x36,
    0x2c, 0x04,
    0x10, 0x80, 0xc0,
    0x3c, 0x64,
    0x53, 0x0c,
    0x01, 0x80, 0xa0,
    0x45, 0x1b,
    0x48, 0x08,
    0x53, 0x1d,
    0x39, 0x81, 0x07,
    0x46, 0x0a,
    0x1d, 0x03,
    0x47, 0x49,
    0x37, 0x03,
    0x0e, 0x08,
    0x0a, 0x06,
    0x39, 0x07,
    0x0a, 0x81, 0x36,
    0x19, 0x80, 0xc7,
    0x32, 0x0d,
    0x83, 0x9b, 0x66,
    0x75, 0x0b,
    0x80, 0xc4, 0x8a, 0xbc,
    0x84, 0x2f, 0x8f, 0xd1,
    0x82, 0x47, 0xa1, 0xb9,
    0x82, 0x39, 0x07,
    0x2a, 0x04,
    0x02, 0x60,
    0x26, 0x0a,
    0x46, 0x0a,
    0x28, 0x05,
    0x13, 0x82, 0xb0,
    0x5b, 0x65,
    0x4b, 0x04,
    0x39, 0x07,
    0x11, 0x40,
    0x04, 0x1c,
    0x97, 0xf8, 0x08,
    0x82, 0xf3, 0xa5, 0x0d,
    0x81, 0x1f, 0x31,
    0x03, 0x11,
    0x04, 0x08,
    0x81, 0x8c, 0x89, 0x04,
    0x6b, 0x05,
    0x0d, 0x03,
    0x09, 0x07,
    0x10, 0x93, 0x60,
    0x80, 0xf6, 0x0a,
    0x73, 0x08,
    0x6e, 0x17,
    0x46, 0x80, 0x9a,
    0x14, 0x0c,
    0x57, 0x09,
    0x19, 0x80, 0x87,
    0x81, 0x47, 0x03,
    0x85, 0x42, 0x0f,
    0x15, 0x85, 0x50,
    0x2b, 0x80, 0xd5,
    0x2d, 0x03,
    0x1a, 0x04,
    0x02, 0x81, 0x70,
    0x3a, 0x05,
    0x01, 0x85, 0x00,
    0x80, 0xd7, 0x29,
    0x4c, 0x04,
    0x0a, 0x04,
    0x02, 0x83, 0x11,
    0x44, 0x4c,
    0x3d, 0x80, 0xc2,
    0x3c, 0x06,
    0x01, 0x04,
    0x55, 0x05,
    0x1b, 0x34,
    0x02, 0x81, 0x0e,
    0x2c, 0x04,
    0x64, 0x0c,
    0x56, 0x0a,
    0x0d, 0x03,
    0x5d, 0x03,
    0x3d, 0x39,
    0x1d, 0x0d,
    0x2c, 0x04,
    0x09, 0x07,
    0x02, 0x0e,
    0x06, 0x80, 0x9a,
    0x83, 0xd6, 0x0a,
    0x0d, 0x03,
    0x0b, 0x05,
    0x74, 0x0c,
    0x59, 0x07,
    0x0c, 0x14,
    0x0c, 0x04,
    0x38, 0x08,
    0x0a, 0x06,
    0x28, 0x08,
    0x1e, 0x52,
    0x77, 0x03,
    0x31, 0x03,
    0x80, 0xa6, 0x0c,
    0x14, 0x04,
    0x03, 0x05,
    0x03, 0x0d,
    0x06, 0x85, 0x6a,
];
