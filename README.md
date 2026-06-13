# patternkit

Maranget's matrix algorithm for pattern match exhaustiveness and reachability checking over arbitrary ASTs, for building compilers in Rust. Also compiles matches to splitting trees for code generation.

## Checking

```rust
use patternkit::{check, Arm, Pat, Signature};

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

`tree::compile` lowers a match to a splitting tree for code generation, reusing the same `Signature` and `Pat`.

```rust
use patternkit::tree::{compile, Tree};
use patternkit::{Pat, Signature};

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

A `Tree` is `Switch` nodes, each naming the `Occurrence` (path into the scrutinee) it inspects, bottoming out at `Leaf` actions. Nested constructors descend into child occurrences. Earlier arms win, and no value is tested twice. A complete constructor set carries no `default`, open types (integers, strings) keep one.

Leaves carry the host's action, not bindings. Recover them by walking the source pattern against the occurrences: `tree::occurrences(pat)` lists the occurrence of every wildcard in order. Column selection is pluggable via `compile_with` (default `leftmost`).

See `examples/splitting.rs`.

## License

MIT
