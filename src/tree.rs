//! Compiling pattern matches to splitting trees.
//!
//! The second half of Maranget's work ("Compiling Pattern Matching to Good
//! Decision Trees", 2008), built on the same [`Signature`] and [`Pat`] the
//! checker uses.
//!
//! Where [`check`](crate::check) decides reachability and exhaustiveness,
//! [`compile`] turns a column of arms into a [`Tree`]: a nest of
//! constructor switches that scrutinizes each sub-value at most once and
//! tests no constructor twice. Each switch names the [`Occurrence`] it
//! inspects (a path into the scrutinee), so nested constructors are
//! deconstructed by descending into child occurrences.
//!
//! The tree is name-agnostic, like the rest of the crate: a leaf carries
//! the host's action for the winning arm, not its bindings. The host
//! recovers bindings by walking its own pattern against the occurrences.
//! [`occurrences`] gives the occurrence of every wildcard in left-to-right
//! order, ready to zip against the variables the host recorded when it
//! lowered the pattern.
//!
//! ```
//! use patternkit::{Pat, Signature};
//! use patternkit::tree::{compile, Tree};
//!
//! struct Bools;
//! impl Signature for Bools {
//!     type Con = bool;
//!     fn arity(&self, _: &bool) -> usize { 0 }
//!     fn siblings(&self, _: &bool) -> Option<Vec<bool>> { Some(vec![false, true]) }
//! }
//!
//! // match b of true => 0 | false => 1
//! let tree = compile(&Bools, &[
//!     (Pat::Con(true, vec![]), 0),
//!     (Pat::Con(false, vec![]), 1),
//! ]);
//! // switch on the scrutinee, one closed branch per constructor, no default
//! assert!(matches!(tree, Tree::Switch { default: None, .. }));
//! ```

use std::fmt;

use crate::{Pat, Signature};

type Row<C> = Vec<Pat<C>>;

/// A path from the scrutinee to a sub-value: each step is a constructor
/// argument index. The root (the scrutinee itself) is the empty path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Occurrence(pub Vec<usize>);

impl Occurrence {
    /// The scrutinee itself.
    #[must_use]
    pub const fn root() -> Self {
        Self(Vec::new())
    }

    /// The `i`th argument of the constructor at this occurrence.
    #[must_use]
    pub fn child(&self, i: usize) -> Self {
        let mut path = self.0.clone();
        path.push(i);
        Self(path)
    }
}

impl fmt::Display for Occurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, ".");
        }
        for i in &self.0 {
            write!(f, ".{i}")?;
        }
        Ok(())
    }
}

/// One arm of a [`Tree::Switch`]: a constructor and the tree to run when the
/// switched occurrence is headed by it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch<C, A> {
    pub con: C,
    pub tree: Tree<C, A>,
}

/// A compiled match: descend the switches, run the leaf's action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tree<C, A> {
    /// The winning arm: run its action. Bindings are the host's to recover
    /// from the arm's pattern (see [`occurrences`]).
    Leaf(A),
    /// No arm matches. Unreachable when the match is exhaustive, so a host
    /// that trusts [`check`](crate::check) may treat this as impossible.
    Fail,
    /// Inspect the head constructor at `occurrence` and branch. `cases` lists
    /// the constructors the arms test, and `default` covers the rest. It is
    /// `None` exactly when `cases` already names every sibling.
    Switch {
        occurrence: Occurrence,
        cases: Vec<Branch<C, A>>,
        default: Option<Box<Self>>,
    },
}

impl<C: fmt::Display, A: fmt::Display> fmt::Display for Tree<C, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leaf(a) => write!(f, "(leaf {a})"),
            Self::Fail => write!(f, "fail"),
            Self::Switch {
                occurrence,
                cases,
                default,
            } => {
                write!(f, "(switch {occurrence}")?;
                for b in cases {
                    write!(f, " ({} {})", b.con, b.tree)?;
                }
                if let Some(d) = default {
                    write!(f, " (_ {d})")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// The occurrence of every [`Pat::Any`] in `pat`, left to right.
///
/// A host that recorded its variables in the same order while lowering can zip
/// the two to learn where each binder reads from. Wildcards appear too, in
/// position, so keep the lowering and this walk in step.
#[must_use]
pub fn occurrences<C>(pat: &Pat<C>) -> Vec<Occurrence> {
    fn go<C>(pat: &Pat<C>, at: &Occurrence, out: &mut Vec<Occurrence>) {
        match pat {
            Pat::Any => out.push(at.clone()),
            Pat::Con(_, args) => {
                for (i, a) in args.iter().enumerate() {
                    go(a, &at.child(i), out);
                }
            }
        }
    }
    let mut out = Vec::new();
    go(pat, &Occurrence::root(), &mut out);
    out
}

/// Leftmost necessary column: the first column some arm tests with a
/// constructor. The default heuristic, deterministic and matching what a
/// hand-written left-to-right compiler would pick.
#[must_use]
pub fn leftmost<C>(rows: &[Row<C>]) -> usize {
    let width = rows.first().map_or(0, Vec::len);
    (0..width)
        .find(|&c| rows.iter().any(|r| matches!(r[c], Pat::Con(..))))
        .unwrap_or(0)
}

/// Compile arms to a splitting tree with the [`leftmost`] heuristic. Earlier
/// arms win: where two arms can match the same value, the tree runs the
/// first.
#[must_use]
pub fn compile<S: Signature, A: Clone>(sig: &S, arms: &[(Pat<S::Con>, A)]) -> Tree<S::Con, A> {
    compile_with(sig, arms, leftmost)
}

/// Compile arms to a splitting tree, choosing each switch column with `heuristic`.
///
/// The heuristic receives the current pattern matrix and returns a column
/// index, with [`leftmost`] as the default. A smarter heuristic (Maranget's
/// necessity scores) yields a smaller tree without changing its meaning.
#[must_use]
pub fn compile_with<S, A, H>(sig: &S, arms: &[(Pat<S::Con>, A)], heuristic: H) -> Tree<S::Con, A>
where
    S: Signature,
    A: Clone,
    H: Fn(&[Row<S::Con>]) -> usize,
{
    let rows: Vec<Row<S::Con>> = arms.iter().map(|(p, _)| vec![p.clone()]).collect();
    let acts: Vec<A> = arms.iter().map(|(_, a)| a.clone()).collect();
    go(sig, &[Occurrence::root()], &rows, &acts, &heuristic)
}

fn go<S, A, H>(
    sig: &S,
    occ: &[Occurrence],
    rows: &[Row<S::Con>],
    acts: &[A],
    heuristic: &H,
) -> Tree<S::Con, A>
where
    S: Signature,
    A: Clone,
    H: Fn(&[Row<S::Con>]) -> usize,
{
    if rows.is_empty() {
        return Tree::Fail;
    }
    // The first surviving arm whose row is all wildcards matches everything
    // that reaches here, and it is the highest-priority such arm.
    if rows[0].iter().all(|p| matches!(p, Pat::Any)) {
        return Tree::Leaf(acts[0].clone());
    }

    let col = heuristic(rows);
    let present = column_heads(rows, col);
    let complete = present
        .first()
        .and_then(|h| sig.siblings(h))
        .is_some_and(|all| all.iter().all(|h| present.contains(h)));

    let cases = present
        .iter()
        .map(|h| {
            let n = sig.arity(h);
            let mut sub_occ = occ[..col].to_vec();
            sub_occ.extend((0..n).map(|i| occ[col].child(i)));
            sub_occ.extend_from_slice(&occ[col + 1..]);
            let (srows, sacts) = specialize(rows, acts, col, h, n);
            Branch {
                con: h.clone(),
                tree: go(sig, &sub_occ, &srows, &sacts, heuristic),
            }
        })
        .collect();

    let default = (!complete).then(|| {
        let mut docc = occ.to_vec();
        docc.remove(col);
        let (drows, dacts) = defaults(rows, acts, col);
        Box::new(go(sig, &docc, &drows, &dacts, heuristic))
    });

    Tree::Switch {
        occurrence: occ[col].clone(),
        cases,
        default,
    }
}

// Distinct constructor heads tested in `col`, in first-seen order.
fn column_heads<C: Clone + PartialEq>(rows: &[Row<C>], col: usize) -> Vec<C> {
    let mut out: Vec<C> = Vec::new();
    for row in rows {
        if let Pat::Con(h, _) = &row[col] {
            if !out.contains(h) {
                out.push(h.clone());
            }
        }
    }
    out
}

// Rows surviving a match of constructor `h` (arity `n`) at `col`: matching
// heads splice their `n` arguments in place of the column, wildcards splice
// `n` wildcards, other heads drop out.
fn specialize<C: Clone + PartialEq, A: Clone>(
    rows: &[Row<C>],
    acts: &[A],
    col: usize,
    h: &C,
    n: usize,
) -> (Vec<Row<C>>, Vec<A>) {
    let mut orows = Vec::new();
    let mut oacts = Vec::new();
    for (row, act) in rows.iter().zip(acts) {
        let fields = match &row[col] {
            Pat::Con(g, args) if g == h => args.clone(),
            Pat::Con(..) => continue,
            Pat::Any => vec![Pat::Any; n],
        };
        let mut nr = row[..col].to_vec();
        nr.extend(fields);
        nr.extend_from_slice(&row[col + 1..]);
        orows.push(nr);
        oacts.push(act.clone());
    }
    (orows, oacts)
}

// Rows reaching the default branch: those with a wildcard at `col`, with the
// column dropped.
fn defaults<C: Clone, A: Clone>(rows: &[Row<C>], acts: &[A], col: usize) -> (Vec<Row<C>>, Vec<A>) {
    let mut orows = Vec::new();
    let mut oacts = Vec::new();
    for (row, act) in rows.iter().zip(acts) {
        if matches!(row[col], Pat::Any) {
            let mut nr = row[..col].to_vec();
            nr.extend_from_slice(&row[col + 1..]);
            orows.push(nr);
            oacts.push(act.clone());
        }
    }
    (orows, oacts)
}
