use std::fmt;

use maranget::{check, Arm, Pat, Signature, Witness};

#[derive(Clone, Debug, PartialEq)]
enum Con {
    Bool(bool),
    Int(i64),
    Nil,
    Cons,
    Pair,
}

impl fmt::Display for Con {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Nil => write!(f, "Nil"),
            Self::Cons => write!(f, "Cons"),
            Self::Pair => write!(f, "Pair"),
        }
    }
}

struct Lang;

impl Signature for Lang {
    type Con = Con;

    fn arity(&self, con: &Con) -> usize {
        match con {
            Con::Cons | Con::Pair => 2,
            _ => 0,
        }
    }

    fn siblings(&self, con: &Con) -> Option<Vec<Con>> {
        match con {
            Con::Bool(_) => Some(vec![Con::Bool(false), Con::Bool(true)]),
            Con::Nil | Con::Cons => Some(vec![Con::Nil, Con::Cons]),
            Con::Pair => Some(vec![Con::Pair]),
            Con::Int(_) => None,
        }
    }

    fn open_witness(&self, present: &[Con]) -> Option<Con> {
        let all_ints = !present.is_empty() && present.iter().all(|c| matches!(c, Con::Int(_)));
        all_ints.then(|| {
            (0..)
                .map(Con::Int)
                .take(present.len() + 1)
                .find(|c| !present.contains(c))
        })?
    }
}

const fn lit(b: bool) -> Pat<Con> {
    Pat::Con(Con::Bool(b), vec![])
}

const fn nil() -> Pat<Con> {
    Pat::Con(Con::Nil, vec![])
}

fn cons(h: Pat<Con>, t: Pat<Con>) -> Pat<Con> {
    Pat::Con(Con::Cons, vec![h, t])
}

fn missing_str(arms: &[Arm<Con>]) -> Option<String> {
    check(&Lang, arms).missing.map(|w| w.to_string())
}

#[test]
fn complete_bools_are_exhaustive() {
    let r = check(&Lang, &[Arm::new(lit(true)), Arm::new(lit(false))]);
    assert!(r.is_exhaustive());
    assert!(r.unreachable.is_empty());
}

#[test]
fn missing_bool_named() {
    assert_eq!(missing_str(&[Arm::new(lit(true))]), Some("false".into()));
}

#[test]
fn wildcard_exhausts_open_type() {
    let arms = [Arm::new(Pat::Con(Con::Int(0), vec![])), Arm::new(Pat::Any)];
    assert!(check(&Lang, &arms).is_exhaustive());
}

#[test]
fn nested_witness() {
    let arms = [Arm::new(nil()), Arm::new(cons(Pat::Any, nil()))];
    assert_eq!(missing_str(&arms), Some("Cons(_, Cons(_, _))".into()));
}

#[test]
fn covered_arm_is_unreachable() {
    let arms = [
        Arm::new(cons(Pat::Any, Pat::Any)),
        Arm::new(cons(nil(), Pat::Any)),
        Arm::new(nil()),
    ];
    let r = check(&Lang, &arms);
    assert_eq!(r.unreachable, vec![1]);
    assert!(r.is_exhaustive());
}

#[test]
fn guarded_arm_covers_nothing() {
    let arms = [Arm::guarded(cons(Pat::Any, Pat::Any)), Arm::new(nil())];
    assert_eq!(missing_str(&arms), Some("Cons(_, _)".into()));
}

#[test]
fn guarded_arm_still_reachability_checked() {
    let arms = [Arm::new(Pat::Any), Arm::guarded(nil())];
    assert_eq!(check(&Lang, &arms).unreachable, vec![1]);
}

#[test]
fn open_witness_names_fresh_literal() {
    let arms: Vec<_> = (0..3)
        .map(|n| Arm::new(Pat::Con(Con::Int(n), vec![])))
        .collect();
    assert_eq!(missing_str(&arms), Some("3".into()));
}

#[test]
fn single_constructor_recurses_into_fields() {
    let pair = |a, b| Pat::Con(Con::Pair, vec![a, b]);
    let arms = [
        Arm::new(pair(lit(true), Pat::Any)),
        Arm::new(pair(lit(false), lit(false))),
    ];
    assert_eq!(missing_str(&arms), Some("Pair(false, true)".into()));
}

#[test]
fn witness_equality_is_structural() {
    let r = check(&Lang, &[Arm::new(lit(true))]);
    assert_eq!(r.missing, Some(Witness::Con(Con::Bool(false), vec![])));
}
