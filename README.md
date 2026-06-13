# maranget

Maranget's matrix algorithm for pattern match exhaustiveness and reachability checking, parameterized over your AST.

```rust
use maranget::{check, Arm, Pat, Signature};

// Nil | Cons(head, tail)
#[derive(Clone, PartialEq)]
enum Con {
    Nil,
    Cons,
}

struct List;

impl Signature for List {
    type Con = Con;
    fn arity(&self, c: &Con) -> usize {
        match c {
            Con::Cons => 2,
            Con::Nil => 0,
        }
    }
    fn siblings(&self, _: &Con) -> Option<Vec<Con>> {
        Some(vec![Con::Nil, Con::Cons])
    }
}

// match xs of
//   Nil => ...
//   Cons(x, Nil) => ...
let nil = || Pat::Con(Con::Nil, vec![]);
let report = check(
    &List,
    &[
        Arm::new(nil()),
        Arm::new(Pat::Con(Con::Cons, vec![Pat::Any, nil()])),
    ],
);

assert!(!report.is_exhaustive());
// report.missing: Cons(_, Cons(_, _))
// report.unreachable: arm indices covered by earlier arms
// Arm::guarded(..): checked for reachability, contributes no coverage
```

## Splitting trees

The `tree` module is the other half of Maranget's work: compiling a match to a splitting tree (a decision tree) for code generation. It reuses the same `Signature` and `Pat`, so one description of your types drives both exhaustiveness checking and lowering.

```rust
use maranget::tree::{compile, Tree};
use maranget::{Pat, Signature};

// reuse the List signature from above
// match xs of Nil => 0 | Cons(x, Nil) => 1 | Cons(x, Cons(..)) => 2
let arms = [
    (Pat::Con(Con::Nil, vec![]), 0),
    (Pat::Con(Con::Cons, vec![Pat::Any, Pat::Con(Con::Nil, vec![])]), 1),
    (Pat::Con(Con::Cons, vec![Pat::Any, Pat::Con(Con::Cons, vec![Pat::Any, Pat::Any])]), 2),
];
let tree = compile(&List, &arms);
// (switch . (Nil (leaf 0)) (Cons (switch .1 (Nil (leaf 1)) (Cons (leaf 2)))))
```

A `Tree` is a nest of `Switch` nodes (each naming the `Occurrence` it inspects, a path into the scrutinee) bottoming out at `Leaf` actions. Nested constructors are deconstructed by descending into child occurrences. A switch over a complete constructor set carries no `default`; open types (integers, strings) keep one. Earlier arms win, and the tree tests no value twice.

The tree is name-agnostic like the checker: a leaf carries your action for the winning arm, not its bindings. Recover bindings by walking your own pattern against the occurrences; `tree::occurrences(pat)` gives the occurrence of every wildcard in order, ready to zip with the variables you recorded when lowering. Column selection is pluggable via `compile_with` (default: `leftmost`).

See `examples/splitting.rs`.

## License

MIT
