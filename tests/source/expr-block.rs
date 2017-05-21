// rustfmt-array_layout: Block
// rustfmt-fn_call_style: Block
// rustfmt-control_style: Rfc
// Test expressions with block formatting.

fn arrays() {
    [      ];
    let empty = [];

    let foo = [a_long_name, a_very_lng_name, a_long_name];

    let foo = [a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_long_name, a_very_lng_name];

    vec![a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_very_lng_name];

    [a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_long_name, a_very_lng_name, a_very_lng_name]
}

fn arrays() {
    let x = [0,
         1,
         2,
         3,
         4,
         5,
         6,
         7,
         8,
         9,
         0,
         1,
         2,
         3,
         4,
         5,
         6,
         7,
         8,
         9,
         0,
         7,
         8,
         9,
         0,
         1,
         2,
         3,
         4,
         5,
         6,
         7,
         8,
         9,
         0];

    let y = [/* comment */ 1, 2 /* post comment */, 3];

    let xy =    [ strukt  { test123: value_one_two_three_four, turbo: coolio(), } , /* comment  */   1 ];

        let a =WeightedChoice::new(&mut [Weighted {
            weight: x,
            item: 0,
        },
                                  Weighted {
            weight: 1,
            item: 1,
        },
                                  Weighted {
            weight: x,
            item: 2,
        },
                                  Weighted {
            weight: 1,
            item: 3,
        }]);

    let z = [xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyyyyyy, zzzzzzzzzzzzzzzzz, q];

    [ 1 +   3, 4 ,  5, 6, 7, 7, fncall::<Vec<_>>(3-1)]
}

fn function_calls() {
    let items = itemize_list(context.codemap,
                             args.iter(),
                             ")",
                             |item| item.span.lo,
                             |item| item.span.hi,
                             |item| {
                                 item.rewrite(context,
                                              Shape {
                                                  width: remaining_width,
                                                  ..nested_shape
                                              })
                             },
                             span.lo,
                             span.hi);

    itemize_list(context.codemap,
                             args.iter(),
                             ")",
                             |item| item.span.lo,
                             |item| item.span.hi,
                             |item| {
                                 item.rewrite(context,
                                              Shape {
                                                  width: remaining_width,
                                                  ..nested_shape
                                              })
                             },
                             span.lo,
                             span.hi)
}

fn macros() {
    baz!(do_not, add, trailing, commas, inside, of, function, like, macros, even, if_they, are, long);

    baz!(one_item_macro_which_is_also_loooooooooooooooooooooooooooooooooooooooooooooooong);

    let _ = match option {
        None => baz!(function, like, macro_as, expression, which, is, loooooooooooooooong),
        Some(p) => baz!(one_item_macro_as_expression_which_is_also_loooooooooooooooong),
    };
}

fn issue_1450() {
    if selfstate
        .compare_exchandsfasdsdfgsdgsdfgsdfgsdfgsdfgsdfgfsfdsage_weak(
            STATE_PARKED,
            STATE_UNPARKED,
            Release,
            Relaxed,
            Release,
            Relaxed,
        )
        .is_ok() {
        return;
    }
}

fn foo() {
    if real_total <= limit && !pre_line_comments &&
            !items.into_iter().any(|item| item.as_ref().is_multiline()) {
         DefinitiveListTactic::Horizontal
    }
}

fn combine_block() {
    foo(
        Bar {
            x: value,
            y: value2,
        },
    );

    let opt = Some(
        Struct(
            long_argument_one,
            long_argument_two,
            long_argggggggg,
        ),
    );

    do_thing(
        |param| {
            action();
            foo(param)
        },
    );

    do_thing(
        x,
        |param| {
            action();
            foo(param)
        },
    );

    Ok(
        some_function(
            lllllllllong_argument_one,
            lllllllllong_argument_two,
            lllllllllllllllllllllllllllllong_argument_three,
        ),
    );

    foo(
        thing,
        bar(
            param2,
            pparam1param1param1param1param1param1param1param1param1param1aram1,
            param3,
        ),
    );

    foo.map_or(
        || {
            Ok(
                SomeStruct {
                    f1: 0,
                    f2: 0,
                    f3: 0,
                },
            )
        },
    );

    match opt {
        Some(x) => somefunc(anotherfunc(
            long_argument_one,
            long_argument_two,
            long_argument_three,
        )),
        None => Ok(SomeStruct {
            f1: long_argument_one,
            f2: long_argument_two,
            f3: long_argument_three,
        }),
    };

    match x {
        y => func(
            xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx,
        ),
        _ => func(
            x,
            yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy,
            zzz,
        ),
    }
}
