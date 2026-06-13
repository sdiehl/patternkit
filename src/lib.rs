//! Exhaustiveness and usefulness checking for pattern matches: Maranget's
//! matrix algorithm parameterized over the host AST.
//!
//! The host lowers its surface patterns into [`Pat`], a tree of constructor
//! applications and wildcards (variables lower to wildcards: they always
//! match), and describes its type structure through the [`Signature`] trait.
//! [`check`] returns the unreachable arms and, for a non-exhaustive match, a
//! [`Witness`]: a concrete pattern no arm covers, ready to print in a
//! diagnostic. Guarded arms follow the standard rule: a guard may fail at
//! runtime, so the arm covers nothing, but it is still checked for
//! reachability.
//!
//! ```
//! use patternkit::{check, Arm, Pat, Signature, Witness};
//!
//! struct Bools;
//!
//! impl Signature for Bools {
//!     type Con = bool;
//!     fn arity(&self, _: &bool) -> usize {
//!         0
//!     }
//!     fn siblings(&self, _: &bool) -> Option<Vec<bool>> {
//!         Some(vec![false, true])
//!     }
//! }
//!
//! let report = check(&Bools, &[Arm::new(Pat::Con(true, vec![]))]);
//! assert_eq!(report.missing, Some(Witness::Con(false, vec![])));
//! assert!(report.unreachable.is_empty());
//! ```
//!
//! [`tree`] is the companion: it compiles the same arms to a splitting tree
//! (a decision tree) for code generation, deconstructing nested constructors
//! by descending into child occurrences.

use std::fmt;

pub mod tree;

/// How the host language's constructors fit together. `Con` is whatever the
/// host uses to identify a constructor head: an enum, an interned symbol, a
/// (name, arity) pair.
pub trait Signature {
    /// Constructor identifier. Equality must mean "same constructor".
    type Con: Clone + PartialEq;

    /// Number of subpatterns the constructor carries.
    fn arity(&self, con: &Self::Con) -> usize;

    /// The complete set of constructors of `con`'s type, or `None` when the
    /// type cannot be enumerated (integers, strings, floats). Open types are
    /// only exhaustible by a wildcard.
    fn siblings(&self, con: &Self::Con) -> Option<Vec<Self::Con>>;

    /// A concrete witness head for an open type, given the constructors a
    /// match already tests for. Lets diagnostics name a literal the arms
    /// miss (a fresh integer, say) instead of `_`. Default: none, the
    /// witness column stays `_`.
    fn open_witness(&self, present: &[Self::Con]) -> Option<Self::Con> {
        let _ = present;
        None
    }
}

/// A lowered pattern: a wildcard or a constructor applied to subpatterns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pat<C> {
    /// Matches anything: `_`, variables, or-pattern expansions left to the host.
    Any,
    /// A constructor with exactly `arity` subpatterns.
    Con(C, Vec<Self>),
}

/// A concrete pattern no arm covers, the shape of the missing case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Witness<C> {
    /// Any value of this position's type.
    Any,
    /// This constructor, with these argument shapes.
    Con(C, Vec<Self>),
}

impl<C: fmt::Display> fmt::Display for Witness<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(f, "_"),
            Self::Con(c, args) if args.is_empty() => write!(f, "{c}"),
            Self::Con(c, args) => {
                write!(f, "{c}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// One match arm: its lowered pattern and whether a guard can reject it.
#[derive(Clone, Debug)]
pub struct Arm<C> {
    pub pat: Pat<C>,
    pub guarded: bool,
}

impl<C> Arm<C> {
    #[must_use]
    pub const fn new(pat: Pat<C>) -> Self {
        Self {
            pat,
            guarded: false,
        }
    }

    #[must_use]
    pub const fn guarded(pat: Pat<C>) -> Self {
        Self { pat, guarded: true }
    }
}

/// What [`check`] found.
#[derive(Debug, PartialEq, Eq)]
pub struct Report<C> {
    /// Indices of arms no value can reach: earlier unguarded arms cover them.
    pub unreachable: Vec<usize>,
    /// A value no arm matches, if the match is non-exhaustive.
    pub missing: Option<Witness<C>>,
}

impl<C> Report<C> {
    #[must_use]
    pub const fn is_exhaustive(&self) -> bool {
        self.missing.is_none()
    }
}

/// Check one match: reachability of every arm, then exhaustiveness of the
/// whole. Guarded arms are checked for reachability but contribute no
/// coverage, since their guard may fail at runtime.
#[must_use]
pub fn check<S: Signature>(sig: &S, arms: &[Arm<S::Con>]) -> Report<S::Con> {
    let mut matrix: Vec<Row<S::Con>> = Vec::new();
    let mut unreachable = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        let row = vec![arm.pat.clone()];
        if !useful(sig, &matrix, &row) {
            unreachable.push(i);
        }
        if !arm.guarded {
            matrix.push(row);
        }
    }
    let missing = witness(sig, &matrix, 1).map(|mut w| w.swap_remove(0));
    Report {
        unreachable,
        missing,
    }
}

type Row<C> = Vec<Pat<C>>;

// Rows that survive matching constructor `h` in the first column: matching
// heads unfold their arguments, wildcards expand to `arity` wildcards,
// other heads drop out.
fn specialize<S: Signature>(sig: &S, m: &[Row<S::Con>], h: &S::Con) -> Vec<Row<S::Con>> {
    m.iter()
        .filter_map(|row| match &row[0] {
            Pat::Con(g, args) if g == h => Some(args.iter().chain(&row[1..]).cloned().collect()),
            Pat::Con(..) => None,
            Pat::Any => {
                let mut out = vec![Pat::Any; sig.arity(h)];
                out.extend_from_slice(&row[1..]);
                Some(out)
            }
        })
        .collect()
}

// Rows whose first column matches values headed by no tested constructor.
fn defaults<C: Clone>(m: &[Row<C>]) -> Vec<Row<C>> {
    m.iter()
        .filter(|row| matches!(row[0], Pat::Any))
        .map(|row| row[1..].to_vec())
        .collect()
}

fn heads<C: Clone + PartialEq>(m: &[Row<C>]) -> Vec<C> {
    let mut out: Vec<C> = Vec::new();
    for row in m {
        if let Pat::Con(h, _) = &row[0] {
            if !out.contains(h) {
                out.push(h.clone());
            }
        }
    }
    out
}

// Maranget's usefulness: does `row` match some value the matrix misses?
fn useful<S: Signature>(sig: &S, m: &[Row<S::Con>], row: &[Pat<S::Con>]) -> bool {
    let Some((head, rest)) = row.split_first() else {
        return m.is_empty();
    };
    match head {
        Pat::Con(h, args) => {
            let r2: Row<S::Con> = args.iter().chain(rest).cloned().collect();
            useful(sig, &specialize(sig, m, h), &r2)
        }
        Pat::Any => {
            let present = heads(m);
            let complete = present
                .first()
                .and_then(|h| sig.siblings(h))
                .is_some_and(|all| all.iter().all(|h| present.contains(h)));
            if complete {
                present.iter().any(|h| {
                    let mut r2 = vec![Pat::Any; sig.arity(h)];
                    r2.extend_from_slice(rest);
                    useful(sig, &specialize(sig, m, h), &r2)
                })
            } else {
                useful(sig, &defaults(m), rest)
            }
        }
    }
}

fn any_args<S: Signature>(sig: &S, h: &S::Con) -> Witness<S::Con> {
    Witness::Con(h.clone(), vec![Witness::Any; sig.arity(h)])
}

// A `width`-column counterexample to exhaustiveness, or None when covered.
fn witness<S: Signature>(sig: &S, m: &[Row<S::Con>], width: usize) -> Option<Vec<Witness<S::Con>>> {
    if width == 0 {
        return m.is_empty().then(Vec::new);
    }
    let present = heads(m);
    let Some(all) = present.first().and_then(|h| sig.siblings(h)) else {
        // Open type (or no heads at all): a wildcard row is the only cover,
        // and the host may know a concrete literal the arms miss.
        let head = sig
            .open_witness(&present)
            .map_or(Witness::Any, |c| any_args(sig, &c));
        let mut out = vec![head];
        out.extend(witness(sig, &defaults(m), width - 1)?);
        return Some(out);
    };
    if let Some(missing) = all.iter().find(|h| !present.contains(h)) {
        let mut out = vec![any_args(sig, missing)];
        out.extend(witness(sig, &defaults(m), width - 1)?);
        return Some(out);
    }
    for h in &present {
        let n = sig.arity(h);
        if let Some(mut w) = witness(sig, &specialize(sig, m, h), n + width - 1) {
            let tail = w.split_off(n);
            let mut out = vec![Witness::Con(h.clone(), w)];
            out.extend(tail);
            return Some(out);
        }
    }
    None
}
