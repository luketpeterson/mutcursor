#![doc = include_str!("../README.md")]

#![no_std]

use core::ptr::NonNull;
use core::mem::MaybeUninit;
use core::marker::PhantomData;

#[cfg(not(feature = "no_std"))]
mod mut_cursor_vec;
#[cfg(not(feature = "no_std"))]
pub use mut_cursor_vec::*;

#[cfg(not(feature = "no_std"))]
mod rooted_vec;
#[cfg(not(feature = "no_std"))]
pub use rooted_vec::*;

/// Stores a stack of `&mut` references, only allowing access to the top element on the stack
///
/// The `MutCursor` stores `N` `&mut T` references, but only allows access to the [top](Self::top)
pub struct MutCursor<'root, T: ?Sized + 'root, const N: usize> {
    cnt: usize, //The last item cannot be removed, so cnt==0 means there is 1 item
    top: usize,
    stack: [MaybeUninit<NonNull<T>>; N],
    phantom: PhantomData<&'root T>,
}

unsafe impl<'a, T, const N: usize> Sync for MutCursor<'a, T, N> where &'a mut T: Sync + Send, T: ?Sized {}
unsafe impl<'a, T, const N: usize> Send for MutCursor<'a, T, N> where &'a mut T: Sync + Send, T: ?Sized {}

impl<'root, T: ?Sized + 'root, const N: usize> MutCursor<'root, T, N> {
    /// Returns a new `MutCursor` with a reference to the specified root
    #[inline]
    pub fn new(root: &'root mut T) -> Self {
        debug_assert!(N > 0);
        let mut stack = Self {
            cnt: 0,
            top: 0,
            stack: [MaybeUninit::uninit(); N],
            phantom: PhantomData::default(),
        };
        unsafe{ *stack.stack.get_unchecked_mut(0) = MaybeUninit::new(NonNull::from(root)); }
        stack
    }
    /// Returns a const reference from the mutable reference on the top of the stack
    #[inline]
    pub fn top(&self) -> &T {
        unsafe{ self.stack.get_unchecked(self.top).assume_init().as_ref() }
    }
    /// Returns the mutable reference on the top of the stack 
    #[inline]
    pub fn top_mut(&mut self) -> &mut T {
        unsafe{ self.top_mut_internal() }
    }
    /// Returns the mutable reference on the top of the stack, consuming the stack
    #[inline]
    pub fn into_mut(mut self) -> &'root mut T {
        unsafe{ self.top_mut_internal() }
    }
    /// Consumes the stack and returns a mutable reference to an object with the `'root` lifetime,
    /// if a closure returns `Ok`, otherwise returns the stack and a custom error value
    ///
    /// This method is useful when you need to call a fallible API with the node, but need the result
    /// of the API to be in the `'root` lifetime so it can outlive the `MutCursor`.
    /// ```
    /// # struct TreeNode {
    /// #   val: usize,
    /// #   next: Option<Box<TreeNode>>
    /// # }
    /// # impl TreeNode {
    /// #   fn new(count: usize) -> Self {
    /// #     if count > 0 {
    /// #       Self {val: count, next: Some(Box::new(Self::new(count-1)))}
    /// #     } else {
    /// #       Self {val: 0, next: None}
    /// #     }
    /// #   }
    /// #   fn traverse(&mut self) -> Option<&mut Self> {
    /// #     self.next.as_mut().map(|boxed| &mut **boxed)
    /// #   }
    /// #   fn is_leaf(&self) -> bool {
    /// #     self.val == 0
    /// #   }
    /// # }
    /// use mutcursor::MutCursor;
    /// let mut tree = TreeNode::new(3);
    ///
    /// let node_stack = MutCursor::<TreeNode, 2>::new(&mut tree);
    /// let node_ref = match node_stack.try_map_into_mut(|top_ref| {
    ///     if top_ref.is_leaf() {
    ///         Ok(top_ref)
    ///     } else {
    ///         Err(top_ref.val)
    ///     }
    /// }) {
    ///     Ok(node) => node,
    ///     Err((mut node_stack, _val)) => {
    ///         if node_stack.depth() > 0 {
    ///             node_stack.backtrack();
    ///         }
    ///         node_stack.into_mut()
    ///     }
    /// };
    /// ```
    #[inline]
    pub fn try_map_into_mut<U, E, F>(mut self, f: F) -> Result<&'root mut U, (Self, E)>
        where for<'r> F: FnOnce(&'r mut T) -> Result<&'r mut U, E>
    {
        let top_ref = unsafe{ self.top_mut_internal() };
        match f(top_ref) {
            Ok(r) => Ok(r),
            Err(e) => Err((self, e))
        }
    }
    /// Returns the number of excess references stored in the stack, which corresponds to the number of
    /// times [backtrack](Self::backtrack) may be called
    #[inline]
    pub fn depth(&self) -> usize {
        self.cnt
    }
    /// Returns the number of references the stack is capable of holding
    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }
    /// Steps deeper into the traversal, pushing a new reference onto the top of the stack
    ///
    /// If the `step_f` closure returns `Some()`, the contained reference is pushed onto the stack and
    /// this method returns `true`.  If the closure returns `None` then the stack is unmodified and this
    /// method returns `false`.
    ///
    /// If the number of references in the stack exceeds the capacity, the reference at the bottom of the
    /// stack will be lost.
    #[inline]
    pub fn advance<F>(&mut self, step_f: F) -> bool
        where F: FnOnce(&'root mut T) -> Option<&'root mut T>
    {
        match step_f(unsafe{ self.top_mut_internal() }) {
            Some(new_node) => {
                unsafe{ self.push(NonNull::from(new_node)); }
                true
            },
            None => false
        }
    }
    /// Pops a reference from the stack, exposing the prior reference as the new [top](Self::top)
    ///
    /// This method will panic if the stack contains only 1 entry
    #[inline]
    pub fn backtrack(&mut self) {
        if self.cnt < 1 {
            panic!("MutCursor must contain valid reference")
        }
        if self.top < 1 {
            self.top = N-1;
        } else {
            self.top -= 1;
        }
        self.cnt -= 1;
    }
    /// Private
    #[inline]
    unsafe fn top_mut_internal(&mut self) -> &'root mut T {
        unsafe{ self.stack[self.top].assume_init().as_mut() }
    }
    /// Private
    #[inline]
    unsafe fn push(&mut self, t: NonNull<T>) {
        if self.top + 1 < N {
            self.top = self.top + 1;
        } else {
            self.top = 0;
        }
        *self.stack.get_unchecked_mut(self.top) = MaybeUninit::new(t);
        if self.cnt < N-1 {
            self.cnt += 1;
        }
    }
}

impl<'root, T: ?Sized, const N: usize> core::ops::Deref for MutCursor<'root, T, N> {
    type Target = T;
    fn deref(&self) -> &T {
        self.top()
    }
}

impl<'root, T: ?Sized, const N: usize> core::ops::DerefMut for MutCursor<'root, T, N> {
    fn deref_mut(&mut self) -> &mut T {
        self.top_mut()
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use std::*;
    use std::boxed::*;
    use std::vec::Vec;

    use crate::*;

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

    #[test]
    fn basics() {
        let mut tree = TreeNode::new(10);
        let mut node_stack = MutCursor::<TreeNode, 7>::new(&mut tree);

        while node_stack.advance(|node| {
            node.traverse()
        }) {}

        assert_eq!(node_stack.top().val, 0);
        assert_eq!(node_stack.depth(), 6);

        node_stack.backtrack();
        assert_eq!(node_stack.top().val, 1);
        assert_eq!(node_stack.depth(), 5);

        node_stack.backtrack();
        node_stack.backtrack();
        node_stack.backtrack();
        assert_eq!(node_stack.top().val, 4);
        assert_eq!(node_stack.depth(), 2);

        while node_stack.advance(|node| {
            node.traverse()
        }) {}
        assert_eq!(node_stack.top().val, 0);
        assert_eq!(node_stack.depth(), 6);

        node_stack.backtrack();
        node_stack.backtrack();
        node_stack.backtrack();
        node_stack.backtrack();
        node_stack.backtrack();
        node_stack.backtrack();
        assert_eq!(node_stack.top().val, 6);
        assert_eq!(node_stack.depth(), 0);

        assert_eq!(node_stack.into_mut().val, 6);
    }

    #[test]
    fn try_to_escape_map_closure() {

        let mut tree = TreeNode::new(3);

        // 1-element node_stack is just a more restrictive `&mut`
        let node_stack = MutCursor::<TreeNode, 1>::new(&mut tree);

        let mut _poison: &mut TreeNode;
        match node_stack.try_map_into_mut(|node| -> Result<&mut TreeNode, &mut TreeNode> {
            //_poison = node; //Good.  Can't escape that way

            //Err(node) //Good.  Can't escape that way either

            Ok(node)
        }) {
            Ok(_r) => {},
            Err(_e) => {}
        }
    }

    use std::{thread, thread::ScopedJoinHandle};
    #[test]
    fn multi_thread_test() {

        let thread_cnt = 128;
        let mut data: Vec<TreeNode> = vec![];
        for _ in 0..thread_cnt {
            data.push(TreeNode::new(10));
        }
        let mut data_refs: Vec<&mut TreeNode> = data.iter_mut().collect();

        thread::scope(|scope| {

            let mut threads: Vec<ScopedJoinHandle<()>> = Vec::with_capacity(thread_cnt);

            //Spawn all the threads
            for _ in 0..thread_cnt {
                let tree = data_refs.pop().unwrap();
                let mut node_stack = MutCursor::<TreeNode, 7>::new(tree);

                let thread = scope.spawn(move || {

                    while node_stack.advance(|node| {
                        node.traverse()
                    }) {}

                    assert_eq!(node_stack.top().val, 0);
                    assert_eq!(node_stack.depth(), 6);

                    node_stack.backtrack();
                    assert_eq!(node_stack.top().val, 1);
                    assert_eq!(node_stack.depth(), 5);

                    node_stack.backtrack();
                    node_stack.backtrack();
                    node_stack.backtrack();
                    assert_eq!(node_stack.top().val, 4);
                    assert_eq!(node_stack.depth(), 2);

                    while node_stack.advance(|node| {
                        node.traverse()
                    }) {}
                    assert_eq!(node_stack.top().val, 0);
                    assert_eq!(node_stack.depth(), 6);

                    node_stack.backtrack();
                    node_stack.backtrack();
                    node_stack.backtrack();
                    node_stack.backtrack();
                    node_stack.backtrack();
                    node_stack.backtrack();
                    assert_eq!(node_stack.top().val, 6);
                    assert_eq!(node_stack.depth(), 0);

                    assert_eq!(node_stack.into_mut().val, 6);
                });
                threads.push(thread);
            };

            //Wait for them to finish
            for thread in threads {
                thread.join().unwrap();
            }
        });
    }
}
