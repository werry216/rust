const DATA: &'static [u8] = &[
    0x42, 0x50, 0x54, 0x44,  //type
    0x23, 0x00, 0x00, 0x00,  //size
    0x00, 0x00, 0x04, 0x00,  //flags
    0xEC, 0x0C, 0x00, 0x00,  //id
    0x00, 0x00, 0x00, 0x00,  //revision
    0x2B, 0x00,  //version
    0x00, 0x00,  //unknown
    0x42, 0x50, 0x54, 0x4E,  //field type
    0x1D, 0x00,  //field size
    0x19, 0x00, 0x00, 0x00,  //decompressed field size
    0x75, 0xc5, 0x21, 0x0d, 0x00, 0x00, 0x08, 0x05, 0xd1, 0x6c,  //field data (compressed)
    0x6c, 0xdc, 0x57, 0x48, 0x3c, 0xfd, 0x5b, 0x5c, 0x02, 0xd4,  //field data (compressed)
    0x6b, 0x32, 0xb5, 0xdc, 0xa3  //field data (compressed)
];
