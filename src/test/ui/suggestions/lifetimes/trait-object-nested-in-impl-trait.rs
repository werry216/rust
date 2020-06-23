trait Foo {}
impl<'a, T: Foo> Foo for &'a T {}
impl<T: Foo + ?Sized> Foo for Box<T> {}

struct Iter<'a, T> {
    current: Option<Box<dyn Foo + 'a>>,
    remaining: T,
}

impl<'a, T> Iterator for Iter<'a, T>
where
    T: Iterator,
    T::Item: Foo + 'a,
{
    type Item = Box<dyn Foo + 'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.current.take();
        self.current = Box::new(self.remaining.next()).map(|f| Box::new(f) as _);
        result
    }
}

struct Bar(Vec<Box<dyn Foo>>);

impl Bar {
    fn iter(&self) -> impl Iterator<Item = Box<dyn Foo>> {
        Iter {
            current: None,
            remaining: self.0.iter(), //~ ERROR cannot infer an appropriate lifetime
        }
    }
}

struct Baz(Vec<Box<dyn Foo>>);

impl Baz {
    fn iter(&self) -> impl Iterator<Item = Box<dyn Foo>> + '_ {
        Iter {
            current: None,
            remaining: self.0.iter(), //~ ERROR cannot infer an appropriate lifetime
        }
    }
}

struct Bat(Vec<Box<dyn Foo>>);

impl Bat {
    fn iter<'a>(&'a self) -> impl Iterator<Item = Box<dyn Foo>> + 'a {
        Iter {
            current: None,
            remaining: self.0.iter(), //~ ERROR cannot infer an appropriate lifetime
        }
    }
}

struct Ban(Vec<Box<dyn Foo>>);

impl Ban {
    fn iter<'a>(&'a self) -> impl Iterator<Item = Box<dyn Foo>> {
        Iter {
            current: None,
            remaining: self.0.iter(), //~ ERROR cannot infer an appropriate lifetime
        }
    }
}

fn main() {}
