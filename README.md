
# mutcursor

This crate provides types to safely store mutable references to parent nodes, for backtracking during traversal of tree & graph structures.

[`MutCursor`] is more efficient because it avoids dynamic allocation, while [`MutCursorVec`] provides for an arbitrarily deep stack.

[`MutCursorRootedVec`] supports mutable references to a separate root type and a different leaf type, for example borrowing a node from an owned container or dereferencing a smart-pointer.

The [`unique`] module provides types to work with [`Rc`] or [`Arc`] pointers as the root of a `MutCursorRootedVec`.

## Usage
```rust
use mutcursor::MutCursor;

let mut tree = TreeNode::new(5);
let mut node_stack = MutCursor::<TreeNode, 2>::new(&mut tree);

// Traverse to the last node
while node_stack.advance(|node| {
    node.traverse()
}) {}

assert_eq!(node_stack.top().val, 0);
assert_eq!(node_stack.depth(), 1);

node_stack.backtrack();
assert_eq!(node_stack.top().val, 1);
assert_eq!(node_stack.depth(), 0);

/// A simple stand-in for a recursive graph structure
struct TreeNode {
    val: usize,
    next: Option<Box<TreeNode>>
}
impl TreeNode {
    fn new(count: usize) -> Self {
        if count > 0 {
            Self {val: count, next: Some(Box::new(Self::new(count-1)))}
        } else {
            Self {val: 0, next: None}
        }
    }
    fn traverse(&mut self) -> Option<&mut Self> {
        self.next.as_mut().map(|boxed| &mut **boxed)
    }
}
```

## Alternative(s)

This crate basically does the same thing as [generic-cursors](https://crates.io/crates/generic-cursors). However, there are several reasons to choose this crate:

1. The fixed-size stack used by [`MutCursor`] has lower overhead than a [`Vec`], and can be used in a `no_std` environment where dynamic allocation may be unavailable.

2. The [`MutCursor::try_map_into_mut`] API enables some patterns that would be otherwise impossible.

## Safety Thesis

Each `&mut` reference stored by a [`MutCursor`] mutably borrows the reference beneath it in the stack.  The stack root takes a mutable (and therefore exclusive) borrow of the node itself.  Therefore the stack's [`top`] is an exclusive borrow.

You can imagine unrolling tree traversal into something like the code below, but this isn't amenable to looping.  In essence each `level` variable is preserved, but inaccessible because the level above is mutably borrowing it.  The [`MutCursor`] object contains all the `level` variables but only provides access to the [`top`].

```rust ignore
let level_1 = &mut root;
{
    let level_2 = level_1.traverse().unwrap();
    {
        match level_2.traverse() {
            Some(level_3) => {} // Do something with level_3
            None => {} // Fall back to work level_2 or level_1
        }
    }
}
```

## Future Work

#### Macro to define cursor types

In the current design of [`MutCursorRootedVec`], there is a predefined pattern prescribing where `RootT` and `NodeT` types may exist on the stack.  However, we may find it necessary in the future to support more than two types, in a more flexible pattern.  It seems that a macro to define a bespoke cursor type is the best solutuion.

#### Internal enum for multiple-type support at runtime

For ultimate flexibility, we would want all the references to be stored by the stack as in an enum over the possible reference types.  However, if the user provided an enum as a type parameter to a cursor type, the result result would be double-indirection.  Therefore the enum behavior would need to be internal to the MutCursor.  Deriving a MutCursor from a user's enum type feels like a friendly way to define the types a cursor type is capable of storing.

## Acknowledgements

[Frank Steffahn](https://github.com/steffahn) identified soundness issues in prior versions of this crate and made vast improvements to the design of [`MutCursorRootedVec`].

[`MutCursor`]: https://docs.rs/mutcursor/latest/mutcursor/struct.MutCursor.html
[`top`]: https://docs.rs/mutcursor/latest/mutcursor/struct.MutCursor.html#method.top
[`MutCursor::try_map_into_mut`]: https://docs.rs/mutcursor/latest/mutcursor/struct.MutCursor.html#method.try_map_into_mut
[`MutCursorVec`]: https://docs.rs/mutcursor/latest/mutcursor/struct.MutCursorVec.html
[`MutCursorRootedVec`]: https://docs.rs/mutcursor/latest/mutcursor/struct.MutCursorRootedVec.html
[`unique`]: https://docs.rs/mutcursor/latest/mutcursor/unique/index.html
[`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
[`Rc`]: https://doc.rust-lang.org/std/rc/struct.Rc.html
[`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html