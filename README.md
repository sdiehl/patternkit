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

## License

MIT
