use patternkit::tree::{compile, occurrences, Branch, Occurrence, Tree};
use patternkit::{Pat, Signature};

#[derive(Clone, Debug, PartialEq)]
enum Con {
    Bool(bool),
    Int(i64),
    Nil,
    Cons,
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
}

const fn nil() -> Pat<Con> {
    Pat::Con(Con::Nil, vec![])
}

fn cons(h: Pat<Con>, t: Pat<Con>) -> Pat<Con> {
    Pat::Con(Con::Cons, vec![h, t])
}

fn occ(path: &[usize]) -> Occurrence {
    Occurrence(path.to_vec())
}

// Walk a tree on a constructor value, returning the action its leaf carries.
// Mirrors what a host runtime does: read the head at each occurrence, follow
// the matching branch (or the default), bottom out at a leaf.
fn run(tree: &Tree<Con, usize>, val: &Pat<Con>) -> Option<usize> {
    match tree {
        Tree::Leaf(a) => Some(*a),
        Tree::Fail => None,
        Tree::Switch {
            occurrence,
            cases,
            default,
        } => {
            let Pat::Con(head, _) = sub(val, &occurrence.0) else {
                return None;
            };
            cases.iter().find(|b| &b.con == head).map_or_else(
                || default.as_deref().and_then(|d| run(d, val)),
                |b| run(&b.tree, val),
            )
        }
    }
}

fn sub<'a>(val: &'a Pat<Con>, path: &[usize]) -> &'a Pat<Con> {
    path.iter().fold(val, |v, &i| match v {
        Pat::Con(_, args) => &args[i],
        Pat::Any => v,
    })
}

#[test]
fn closed_switch_has_no_default() {
    let tree = compile(
        &Lang,
        &[
            (Pat::Con(Con::Bool(true), vec![]), 0),
            (Pat::Con(Con::Bool(false), vec![]), 1),
        ],
    );
    let Tree::Switch {
        occurrence,
        cases,
        default,
    } = &tree
    else {
        panic!("expected a switch");
    };
    assert_eq!(*occurrence, Occurrence::root());
    assert_eq!(cases.len(), 2);
    assert!(default.is_none());
}

#[test]
fn open_switch_keeps_a_default() {
    // Integers are open: the switch must keep a fallthrough.
    let tree = compile(&Lang, &[(Pat::Con(Con::Int(0), vec![]), 0), (Pat::Any, 1)]);
    let Tree::Switch { cases, default, .. } = &tree else {
        panic!("expected a switch");
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(default.as_deref(), Some(&Tree::Leaf(1)));
}

#[test]
fn nested_constructors_descend_into_child_occurrences() {
    // match xs of Cons(_, Nil) => 0 | _ => 1
    let tree = compile(&Lang, &[(cons(Pat::Any, nil()), 0), (Pat::Any, 1)]);
    // Outer switch on the scrutinee.
    let Tree::Switch {
        occurrence,
        cases,
        default,
    } = &tree
    else {
        panic!("expected a switch");
    };
    assert_eq!(*occurrence, Occurrence::root());
    assert_eq!(default.as_deref(), Some(&Tree::Leaf(1)));
    // The Cons branch switches on the tail, occurrence .1 .
    let cons_branch = cases.iter().find(|b| b.con == Con::Cons).unwrap();
    let Tree::Switch {
        occurrence: inner, ..
    } = &cons_branch.tree
    else {
        panic!("expected an inner switch on the tail");
    };
    assert_eq!(*inner, occ(&[1]));
}

#[test]
fn first_matching_arm_wins() {
    // match xs of Cons(_, _) => 0 | Cons(Nil, _) => 1 | Nil => 2
    let tree = compile(
        &Lang,
        &[
            (cons(Pat::Any, Pat::Any), 0),
            (cons(nil(), Pat::Any), 1),
            (nil(), 2),
        ],
    );
    // A cons of anything routes to arm 0, never the shadowed arm 1.
    assert_eq!(run(&tree, &cons(nil(), nil())), Some(0));
    assert_eq!(run(&tree, &cons(cons(nil(), nil()), nil())), Some(0));
    assert_eq!(run(&tree, &nil()), Some(2));
}

#[test]
fn tree_dispatches_every_constructor() {
    // match xs of Nil => 0 | Cons(_, Nil) => 1 | Cons(_, Cons(_, _)) => 2
    let tree = compile(
        &Lang,
        &[
            (nil(), 0),
            (cons(Pat::Any, nil()), 1),
            (cons(Pat::Any, cons(Pat::Any, Pat::Any)), 2),
        ],
    );
    assert_eq!(run(&tree, &nil()), Some(0));
    assert_eq!(run(&tree, &cons(nil(), nil())), Some(1));
    assert_eq!(run(&tree, &cons(nil(), cons(nil(), nil()))), Some(2));
}

#[test]
fn occurrences_track_binding_paths() {
    // Cons(x, Cons(y, ys)): x reads .0, y reads .1.0, ys reads .1.1 .
    let pat = cons(Pat::Any, cons(Pat::Any, Pat::Any));
    assert_eq!(
        occurrences(&pat),
        vec![occ(&[0]), occ(&[1, 0]), occ(&[1, 1])]
    );
}

#[test]
fn occurrence_displays_as_a_path() {
    assert_eq!(Occurrence::root().to_string(), ".");
    assert_eq!(occ(&[1, 0]).to_string(), ".1.0");
}

#[test]
fn tree_displays_as_an_sexpr() {
    let tree: Tree<Con, usize> = Tree::Switch {
        occurrence: Occurrence::root(),
        cases: vec![Branch {
            con: Con::Nil,
            tree: Tree::Leaf(0),
        }],
        default: Some(Box::new(Tree::Leaf(1))),
    };
    assert_eq!(tree.to_string(), "(switch . (Nil (leaf 0)) (_ (leaf 1)))");
}

// Con needs Display for the sexpr test.
impl std::fmt::Display for Con {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Nil => write!(f, "Nil"),
            Self::Cons => write!(f, "Cons"),
        }
    }
}
