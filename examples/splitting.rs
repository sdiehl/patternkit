// Compiling a list match to a splitting tree, then printing it. The same
// signature drives both `check` (exhaustiveness) and `tree::compile` (code
// generation), so a host reuses one description of its types for both.

use std::fmt;

use maranget::tree::{compile, occurrences};
use maranget::{Pat, Signature};

#[derive(Clone, Debug, PartialEq)]
enum Con {
    Nil,
    Cons,
}

impl fmt::Display for Con {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "Nil"),
            Self::Cons => write!(f, "Cons"),
        }
    }
}

struct List;

impl Signature for List {
    type Con = Con;

    fn arity(&self, con: &Con) -> usize {
        match con {
            Con::Cons => 2,
            Con::Nil => 0,
        }
    }

    fn siblings(&self, _: &Con) -> Option<Vec<Con>> {
        Some(vec![Con::Nil, Con::Cons])
    }
}

const fn nil() -> Pat<Con> {
    Pat::Con(Con::Nil, vec![])
}

fn cons(h: Pat<Con>, t: Pat<Con>) -> Pat<Con> {
    Pat::Con(Con::Cons, vec![h, t])
}

fn main() {
    // match xs of
    //   Nil               => 0
    //   Cons(x, Nil)      => 1
    //   Cons(x, Cons(..)) => 2
    let arms = [
        (nil(), 0),
        (cons(Pat::Any, nil()), 1),
        (cons(Pat::Any, cons(Pat::Any, Pat::Any)), 2),
    ];
    let tree = compile(&List, &arms);
    println!("splitting tree:\n  {tree}\n");

    // Arm 1 binds x at the head of the scrutinee. Its wildcards, left to
    // right, read from these occurrences:
    println!("arm 1 binders read from: {:?}", occurrences(&arms[1].0));
}
