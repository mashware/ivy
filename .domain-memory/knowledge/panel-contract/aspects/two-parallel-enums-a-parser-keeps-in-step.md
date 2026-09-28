---
id: asp_two-parallel-enums-a-parser-keeps-in-step_s6cjan
slug: two-parallel-enums-a-parser-keeps-in-step
name: >-
  A field enum beside a value enum is one thing split in two, and the split
  fails in the arm that answers for the pair that «cannot happen»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-19T14:29:08.199Z'
updated_at: '2026-09-19T14:29:08.199Z'
last_verified: '2026-09-19T14:29:08.199Z'
file_paths:
  - crates/panel-contract/src/patch.rs
symbols:
  - 'Op::Set'
  - Scalar
  - parse_ops
  - set_op
content_hashes: {}
tags:
  - type-design
  - panel-patch
  - d47
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The patch vocabulary first had `Op::Set { field: Field, value: Value }`, and `parse_ops` always paired them correctly** — so no real input could produce a mismatch. But both types were `pub` and re-exported, the pair was constructible, and the applying function had to answer for it. It did, with four catch-alls:

```rust
(Field::Phase, _) => panel.phase = None,
```

So a `Set { Header, Value::Phase(..) }` **cleared a field nobody had named**, silently. And `Value::Nothing` existed only to say «null» for four different types whose nulls mean different things — absence for two of them, the default for the other two — so every reader had to go and find `fn set` to learn which.

> **One enum carrying its own value removes the failure rather than guarding it.**
>
> ```rust
> pub enum Scalar {
>     Phase(Option<String>),
>     Attention(Option<Attention>),
>     Header(bool),
>     StaleAfter(Duration),
> }
> ```

The JSON null is resolved **at the parse**, where the field is known, so the two fields whose absence produces a value carry no `Option` at all, and the applying function is four total arms with no catch-all. `Field::name()` went with it: its only caller was the mismatch message, and the wire name is already in scope there as the string that was matched on.

## What makes it worth a card is how it was found

**Two reviewers — a type-design agent and an idiom agent, blinded to each other and to the design's prose — reported the same defect with the same fix, independently.** Neither the compiler, the suite, nor any reading of the diff had anything to say about it: the code was correct for every input the parser can produce, and wrong for the type it declared.

**The general shape**: when a parser is the only thing keeping two types in step, the invariant lives in the parser and not in the types — and the place it fails is the arm somebody had to write for the state they believed impossible. That arm is never a refusal, because a refusal would need a reason; it is always a silent default.

## The sibling mistake, one round later, in the same file

The same delivery closed a duplicate-key hole in `Index::inserted_at` and wrote a docblock on `remember` arguing that the *other* caller — a replacement — could not collide, «because it has already taken the old line's key out of the index, so a collision would be with a different line and is the insert's case». That is the case, and it is exactly why it had to be refused there too: replacing the line keyed `mr-1` with one keyed `mr-3`, on a panel that already has an `mr-3`, produced the very two-lines-one-key state the insert refuses. **A comment arguing that a guard is unnecessary somewhere is worth the same suspicion as a catch-all arm**: both are prose standing where a check would go.

## Where it lives

`crates/panel-contract/src/patch.rs` — `Op::Set(Scalar)`, `set_op`, `fn set`, and `Index::remember`. **D47** in `docs/02-decisions.md` records both the collapse and the replacement collision in its discarded table.
