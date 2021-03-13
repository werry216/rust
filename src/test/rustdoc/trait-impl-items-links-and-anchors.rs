// ignore-tidy-linelength

pub trait MyTrait {
    type Assoc;
    const VALUE: u32;
    fn trait_function(&self);
    fn defaulted(&self) {}
    fn defaulted_override(&self) {}
}


impl MyTrait for String {
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-1"]//a[@class="type"]/@href' #associatedtype.Assoc
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-1"]//a[@class="anchor"]/@href' #associatedtype.Assoc-1
    type Assoc = ();
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-1"]//a[@class="constant"]/@href' #associatedconstant.VALUE
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-1"]//a[@class="anchor"]/@href' #associatedconstant.VALUE-1
    const VALUE: u32 = 5;
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function"]//a[@class="fnname"]/@href' #tymethod.trait_function
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function"]//a[@class="anchor"]/@href' #method.trait_function
    fn trait_function(&self) {}
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-1"]//a[@class="fnname"]/@href' #method.defaulted_override
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-1"]//a[@class="anchor"]/@href' #method.defaulted_override-1
    fn defaulted_override(&self) {}
}

impl MyTrait for Vec<u8> {
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-2"]//a[@class="type"]/@href' #associatedtype.Assoc
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-2"]//a[@class="anchor"]/@href' #associatedtype.Assoc-2
    type Assoc = ();
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-2"]//a[@class="constant"]/@href' #associatedconstant.VALUE
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-2"]//a[@class="anchor"]/@href' #associatedconstant.VALUE-2
    const VALUE: u32 = 5;
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function"]//a[@class="fnname"]/@href' #tymethod.trait_function
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function-1"]//a[@class="anchor"]/@href' #method.trait_function-1
    fn trait_function(&self) {}
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-2"]//a[@class="fnname"]/@href' #method.defaulted_override
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-2"]//a[@class="anchor"]/@href' #method.defaulted_override-2
    fn defaulted_override(&self) {}
}

impl MyTrait for MyStruct {
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-3"]//a[@class="type"]/@href' #associatedtype.Assoc
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedtype.Assoc-3"]//a[@class="anchor"]/@href' #associatedtype.Assoc-3
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="associatedtype.Assoc"]//a[@class="type"]/@href' ../trait_impl_items_links_and_anchors/trait.MyTrait.html#associatedtype.Assoc
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="associatedtype.Assoc"]//a[@class="anchor"]/@href' #associatedtype.Assoc
    type Assoc = bool;
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-3"]//a[@class="constant"]/@href' #associatedconstant.VALUE
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="associatedconstant.VALUE-3"]//a[@class="anchor"]/@href' #associatedconstant.VALUE-3
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="associatedconstant.VALUE"]//a[@class="constant"]/@href' ../trait_impl_items_links_and_anchors/trait.MyTrait.html#associatedconstant.VALUE
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="associatedconstant.VALUE"]//a[@class="anchor"]/@href' #associatedconstant.VALUE
    const VALUE: u32 = 20;
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function-2"]//a[@class="fnname"]/@href' #tymethod.trait_function
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.trait_function-2"]//a[@class="anchor"]/@href' #method.trait_function-2
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="method.trait_function"]//a[@class="fnname"]/@href' ../trait_impl_items_links_and_anchors/trait.MyTrait.html#tymethod.trait_function
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="method.trait_function"]//a[@class="anchor"]/@href' #method.trait_function
    fn trait_function(&self) {}
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-3"]//a[@class="fnname"]/@href' #method.defaulted_override
    // @has trait_impl_items_links_and_anchors/trait.MyTrait.html '//h4[@id="method.defaulted_override-3"]//a[@class="anchor"]/@href' #method.defaulted_override-3
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="method.defaulted_override"]//a[@class="fnname"]/@href' ../trait_impl_items_links_and_anchors/trait.MyTrait.html#method.defaulted_override
    // @has trait_impl_items_links_and_anchors/struct.MyStruct.html '//h4[@id="method.defaulted_override"]//a[@class="anchor"]/@href' #method.defaulted_override
    fn defaulted_override(&self) {}
}

pub struct MyStruct;
