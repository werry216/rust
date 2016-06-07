#![feature(plugin)]
#![plugin(clippy)]

#![deny(needless_return)]

fn test_end_of_fn() -> bool {
    if true {
        // no error!
        return true;
    }
    return true;
    //~^ ERROR unneeded return statement
    //~| HELP remove `return` as shown
    //~| SUGGESTION true
}

fn test_no_semicolon() -> bool {
    return true
    //~^ ERROR unneeded return statement
    //~| HELP remove `return` as shown
    //~| SUGGESTION true
}

fn test_if_block() -> bool {
    if true {
        return true;
        //~^ ERROR unneeded return statement
        //~| HELP remove `return` as shown
        //~| SUGGESTION true
    } else {
        return false;
        //~^ ERROR unneeded return statement
        //~| HELP remove `return` as shown
        //~| SUGGESTION false
    }
}

fn test_match(x: bool) -> bool {
    match x {
        true => {
            return false;
            //~^ ERROR unneeded return statement
            //~| HELP remove `return` as shown
            //~| SUGGESTION false
        }
        false => {
            return true;
            //~^ ERROR unneeded return statement
            //~| HELP remove `return` as shown
            //~| SUGGESTION true
        }
    }
}

fn test_closure() {
    let _ = || {
        return true;
        //~^ ERROR unneeded return statement
        //~| HELP remove `return` as shown
        //~| SUGGESTION true
    };
}

fn main() {
    let _ = test_end_of_fn();
    let _ = test_no_semicolon();
    let _ = test_if_block();
    let _ = test_match(true);
    test_closure();
}
