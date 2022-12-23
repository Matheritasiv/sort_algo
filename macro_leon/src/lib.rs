use proc_macro::{TokenStream, TokenTree, Literal, Group, Delimiter, Punct, Spacing};

enum State {
    Start, Work, Sep, Stop
}

struct LeonSeq {
    a: usize,
    b: usize,
    s: State,
}

impl LeonSeq {
    fn new(a: usize, b: usize) -> Self {
        LeonSeq { a, b, s: State::Start }
    }
}

impl Iterator for LeonSeq {
    type Item = TokenTree;
    fn next(&mut self) -> Option<Self::Item> {
        match self.s {
            State::Start => {
                self.s = State::Sep;
                Some(TokenTree::Literal(Literal::usize_suffixed(self.a)))
            },
            State::Work => {
                let (c, o) = self.b.overflowing_add(self.a + 1);
                (self.a, self.b) = (self.b, c);
                self.s = if o { State::Stop } else { State::Sep };
                Some(TokenTree::Literal(Literal::usize_suffixed(self.a)))
            },
            State::Sep => {
                self.s = State::Work;
                Some(TokenTree::Punct(Punct::new(',', Spacing::Alone)))
            },
            State::Stop => None,
        }
    }
}

#[proc_macro]
pub fn gen_leonardo_ind(_: TokenStream) -> TokenStream {
    TokenStream::from(TokenTree::Group(Group::new(
        Delimiter::Bracket, LeonSeq::new(1, 1).collect()
    )))
}
