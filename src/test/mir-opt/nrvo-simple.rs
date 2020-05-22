// EMIT_MIR rustc.nrvo.RenameReturnPlace.diff
fn nrvo(init: fn(&mut [u8; 1024])) -> [u8; 1024] {
    let mut buf = [0; 1024];
    init(&mut buf);
    buf
}

fn main() {
    let _ = nrvo(|buf| { buf[4] = 4; });
}
