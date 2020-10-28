// FIXME(arora-aman) add run-pass once 2229 is implemented

#![feature(capture_disjoint_fields)]
//~^ warning the feature `capture_disjoint_fields` is incomplete
#![feature(rustc_attrs)]

struct Filter {
    div: i32,
}
impl Filter {
    fn allowed(&self, x: i32) -> bool {
        x % self.div == 1
    }
}

struct Data {
    filter: Filter,
    list: Vec<i32>,
}
impl Data {
    fn update(&mut self) {
        // The closure passed to filter only captures self.filter,
        // therefore mutating self.list is allowed.
        self.list.retain(
            #[rustc_capture_analysis]
            |v| self.filter.allowed(*v),
            //~^ ERROR: Capturing self[Deref,(0, 0)] -> ImmBorrow
            //~^^ ERROR: Min Capture self[Deref,(0, 0)] -> ImmBorrow
        );
    }
}

fn main() {
    let mut d = Data { filter: Filter { div: 3 }, list: Vec::new() };

    for i in 1..10 {
        d.list.push(i);
    }

    d.update();
}
