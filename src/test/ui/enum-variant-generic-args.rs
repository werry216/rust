#![feature(type_alias_enum_variants)]

enum Enum<T> { TSVariant(T), SVariant { v: T } }
type Alias<T> = Enum<T>;
type AliasFixed = Enum<()>;

impl<T> Enum<T> {
    fn ts_variant() {
        Self::TSVariant::<()>(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        Self::<()>::TSVariant(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        Self::<()>::TSVariant::<()>(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        //~^^ ERROR type arguments are not allowed on this entity [E0109]
    }

    fn s_variant() {
        Self::SVariant::<()>(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        Self::<()>::SVariant(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        Self::<()>::SVariant::<()>(());
        //~^ ERROR type arguments are not allowed on this entity [E0109]
        //~^^ ERROR type arguments are not allowed on this entity [E0109]
    }
}

fn main() {
    // Tuple struct variant

    Enum::<()>::TSVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]

    Alias::TSVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    Alias::<()>::TSVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]

    AliasFixed::TSVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    AliasFixed::<()>::TSVariant(());
    //~^ ERROR wrong number of type arguments: expected 0, found 1 [E0107]
    AliasFixed::<()>::TSVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    //~^^ ERROR wrong number of type arguments: expected 0, found 1 [E0107]

    // Struct variant

    Enum::<()>::SVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]

    Alias::SVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    Alias::<()>::SVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]

    AliasFixed::SVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    AliasFixed::<()>::SVariant(());
    //~^ ERROR wrong number of type arguments: expected 0, found 1 [E0107]
    AliasFixed::<()>::SVariant::<()>(());
    //~^ ERROR type arguments are not allowed on this entity [E0109]
    //~^^ ERROR wrong number of type arguments: expected 0, found 1 [E0107]
}
