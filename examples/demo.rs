// A toy language with booleans, lists, and unbounded integers, showing the
// three checks: missing-case witnesses, unreachable arms, and guards.

use std::fmt;

use patternkit::{check, Arm, Pat, Signature};

#[derive(Clone, Debug, PartialEq)]
enum Con {
    Bool(bool),
    Int(i64),
    Nil,
    Cons,
}

impl fmt::Display for Con {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Nil => write!(f, "Nil"),
            Self::Cons => write!(f, "Cons"),
        }
    }
}

struct Lang;

impl Signature for Lang {
    type Con = Con;

    fn arity(&self, con: &Con) -> usize {
        match con {
            Con::Cons => 2,
            _ => 0,
        }
    }

    fn siblings(&self, con: &Con) -> Option<Vec<Con>> {
        match con {
            Con::Bool(_) => Some(vec![Con::Bool(false), Con::Bool(true)]),
            Con::Nil | Con::Cons => Some(vec![Con::Nil, Con::Cons]),
            Con::Int(_) => None,
        }
    }

    fn open_witness(&self, present: &[Con]) -> Option<Con> {
        let ints: Vec<i64> = present
            .iter()
            .filter_map(|c| match c {
                Con::Int(n) => Some(*n),
                _ => None,
            })
            .collect();
        if ints.is_empty() || ints.len() != present.len() {
            return None;
        }
        // Among len + 1 candidates one must be fresh.
        (0..)
            .map(Con::Int)
            .take(ints.len() + 1)
            .find(|c| !present.contains(c))
    }
}

const fn nil() -> Pat<Con> {
    Pat::Con(Con::Nil, vec![])
}

fn cons(h: Pat<Con>, t: Pat<Con>) -> Pat<Con> {
    Pat::Con(Con::Cons, vec![h, t])
}

fn main() {
    // match xs of Nil => ... | Cons(x, Nil) => ...
    let r = check(&Lang, &[Arm::new(nil()), Arm::new(cons(Pat::Any, nil()))]);
    println!("lists up to length 1:   missing {}", r.missing.unwrap());

    // match b of true => ... | false => ... | true => ...
    let t = Pat::Con(Con::Bool(true), vec![]);
    let f = Pat::Con(Con::Bool(false), vec![]);
    let r = check(&Lang, &[Arm::new(t.clone()), Arm::new(f), Arm::new(t)]);
    println!("duplicate bool arm:     unreachable {:?}", r.unreachable);

    // match xs of Cons(x, _) if x > 0 => ... | Nil => ...
    let r = check(
        &Lang,
        &[Arm::guarded(cons(Pat::Any, Pat::Any)), Arm::new(nil())],
    );
    println!("guard covers nothing:   missing {}", r.missing.unwrap());

    // match n of 0 => ... | 1 => ... | 2 => ...
    let arms: Vec<_> = (0..3)
        .map(|n| Arm::new(Pat::Con(Con::Int(n), vec![])))
        .collect();
    let r = check(&Lang, &arms);
    println!("ints are open:          missing {}", r.missing.unwrap());
}
