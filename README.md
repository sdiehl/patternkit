# maranget

Pattern match exhaustiveness checking for compilers. [Maranget's matrix algorithm](http://moscova.inria.fr/~maranget/papers/warn/warn.pdf), bring your own AST.

Implement one trait describing your constructors, lower your patterns to a constructor and wildcard tree, and get back the three checks every ADT language needs: missing-case witnesses for non-exhaustive matches (`missing Cons(_, Cons(_, _))`), unreachable arm detection, and guards that cover nothing but are still reachability checked. Open types get fresh-literal witnesses: a match on `0, 1, 2` reports `missing 3`. Zero dependencies, no unsafe.

```rust
struct Bools;

impl Signature for Bools {
    type Con = bool;
    fn arity(&self, _: &bool) -> usize { 0 }
    fn siblings(&self, _: &bool) -> Option<Vec<bool>> { Some(vec![false, true]) }
}

let report = check(&Bools, &[Arm::new(Pat::Con(true, vec![]))]);
assert_eq!(report.missing, Some(Witness::Con(false, vec![])));
```

```bash
cargo add maranget
```

## License

MIT
