// Test tuple litterals

fn foo() {
    let a = (a, a, a, a, a);
    let aaaaaaaaaaaaaaaa = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, aaaaaaaaaaaaaa, aaaaaaaaaaaaaa);
    let aaaaaaaaaaaaaaaaaaaaaa = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
                                  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
                                  aaaaaaaaaaaaaaaaaaaaaaaaa,
                                  aaaa);
    let a = (a,);

    let b = (// This is a comment
             b, // Comment
             b /* Trailing comment */);

    // #1063
    foo(x.0 .0);
}

fn a() {
    ((aaaaaaaa,
      aaaaaaaaaaaaa,
      aaaaaaaaaaaaaaaaa,
      aaaaaaaaaaaaaa,
      aaaaaaaaaaaaaaaa,
      aaaaaaaaaaaaaa),)
}

fn b() {
    ((bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
     bbbbbbbbbbbbbbbbbb)
}

fn issue550() {
    self.visitor.visit_volume(self.level.sector_id(sector), (floor_y,
    if is_sky_flat(ceil_tex) {from_wad_height(self.height_range.1)} else {ceil_y}));
}

fn issue775() {
    if indent {
        let a = mk_object(&[("a".to_string(), Boolean(true)),
                            ("b".to_string(),
                             Array(vec![mk_object(&[("c".to_string(),
                                                     String("\x0c\r".to_string()))]),
                                        mk_object(&[("d".to_string(), String("".to_string()))])]))]);
    }
}
