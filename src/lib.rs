//! Generic equivalence predicate that can handle cyclic, very-deep, very-large, and degenerate
//! graphs.  Extends the algorithm described in the paper [Efficient Nondestructive Equality
//! Checking for Trees and Graphs](https://michaeldadams.org/papers/efficient_equality/).  TODO:
//! Has further enhancements, like ordering comparison ...
#![cfg_attr(
    not(feature = "std"),
    doc = "\n",
    doc = "Note: This crate was built without its `std` feature and some premade items are \
           unavailable, and so custom types must be provided and used with the items of the \
           [`generic`] module, to have cycle-safety and/or deep-safety."
)]
#![cfg_attr(
    all(not(feature = "std"), feature = "alloc"),
    doc = "\n",
    doc = "Note: This crate was built with its `alloc` feature, and so some premade items, \
           that use the [`alloc`](https://doc.rust-lang.org/alloc/) crate, are available."
)]
// Apply the `no_std` attribute unconditionally, to require explicit `use` of non-`core` items.
#![no_std]
#![forbid(unsafe_code)]
// Warn about desired lints that would otherwise be allowed by default.
#![warn(
    // Groups
    future_incompatible,
    nonstandard_style,
    rust_2018_compatibility, // unsure if needed with edition="2018"
    rust_2018_idioms,
    rust_2021_compatibility,
    unused,
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    // Individual lints not included in above groups and desired.
    macro_use_extern_crate,
    meta_variable_misuse,
    // missing_copy_implementations,
    // missing_debug_implementations,
    missing_docs,
    // // missing_doc_code_examples, // maybe someday
    noop_method_call,
    pointer_structural_match,
    single_use_lifetimes, // annoying hits on invisible derived impls
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    // unused_crate_dependencies, // annoying hits for dev-dependencies
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences,
)]
// Exclude (re-allow) undesired lints included in above groups.
#![allow(
    clippy::implicit_return,
    clippy::blanket_clippy_restriction_lints,
    clippy::default_numeric_fallback,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    clippy::shadow_reuse
)]


use core::{
    hash::Hash,
    ops::AddAssign,
};


#[cfg(feature = "std")]
/// Items that are safe for cyclic, degenerate, and very-deep graphs.
pub mod robust;

/// Items that are safe for cyclic and degenerate graphs, but not, by themselves, for very-deep
/// graphs.
pub mod cycle_safe;

#[cfg(feature = "alloc")]
/// Items that are safe for very-deep graphs, but not, by themselves, for cyclic nor degenerate
/// graphs.
pub mod deep_safe;

/// Items that are not safe for cyclic, degenerate, nor very-deep graphs.
pub mod basic;

/// Items that require choosing specific instantiations, which allows customizability beyond the
/// premade functions of the other modules.  Can be used to achieve cycle-safety and/or
/// deep-safety.
pub mod generic;


/// What the algorithm requires from a type, to be applied to it.
pub trait Node
{
    /// Determines when nodes are the same identical node and so can immediately be considered
    /// equivalent without checking their values, edges, nor descendents.  The size of and methods
    /// on this type should be small and very cheap.
    ///
    /// For types where only nodes that are the same object in memory can be considered identical,
    /// pointer/address equality and hashing should be used by defining this type to be `*const T`
    /// where `T` is either `Self` or the primary inner type.  Such pointers are never
    /// dereferenced, and so there is no `unsafe` usage.  (Unfortunately, trying to use `&T` would
    /// cause too many difficulties with lifetimes.  Using `*const T` is valid for the algorithm
    /// because the lifetimes of the `&Self` borrows for the entry-point function calls outlive
    /// such pointers used internally, and so the `Self` objects cannot move during those
    /// lifetimes and so the pointers remain valid.)
    ///
    /// For other types where different `Self` objects can represent the same identical node, some
    /// approach following that should be provided, and the pointer/address approach should not be
    /// used.
    type Id: Eq + Hash + Clone;

    /// Determines what is used to index descendent nodes and to represent the amount of them.
    /// The primitive unsigned integer types, like `usize`, are a common choice, but it may be
    /// anything that satisfies the trait bounds.
    ///
    /// Only `Self::Index::from(0)`, `Self::Index::from(1)`, and `Self::Index::add_assign(index,
    /// 1.into())` are actually used by the algorithm, and so the type does not actually have to
    /// support `From<u8>` for the rest of the `u8` range, and does not actually have to support
    /// `AddAssign` of increments other than the unit value nor of results beyond the maximum
    /// possible amount of edges.
    ///
    /// E.g. for graphs with nodes whose amounts of edges are always smaller than some limit, it
    /// might be desirable, for efficiency, to use an index type smaller than `usize`.  Or for
    /// other node types, it might be more logical or convenient to use an index type that is not
    /// a number.
    type Index: Eq + Ord + AddAssign + From<u8>;

    /// Get the identity of the `self` node.  The result must only be `==` to another node's when
    /// the nodes should be considered identical.
    fn id(&self) -> Self::Id;

    /// Determines how many edges the `self` node has that the algorithm will descend into and
    /// check.  All indices in the range `0.into() .. self.amount_edges()` must be valid to call
    /// [`self.get_edge(index)`](Self::get_edge) with.
    fn amount_edges(&self) -> Self::Index;

    /// Get descendent node by index.  The index must be within the range `0.into()
    /// .. self.amount_edges()`.  The algorithm calls this method, for each index in that range,
    /// to descend into each edge.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.  But since the same implementor controls
    /// [`Self::amount_edges`], and when that is implemented correctly, as it must be, then such
    /// out-of-bounds panics are impossible, as used by the algorithm.
    fn get_edge(
        &self,
        index: &Self::Index,
    ) -> Self;

    /// Check if the nodes are equivalent in their own directly-contained semantically-significant
    /// values ignoring their edges and ignoring their descendent nodes.  This is intended to be
    /// used by [`Self::equiv_modulo_descendents_then_amount_edges`].
    ///
    /// E.g. a node type like:
    ///
    /// ```rust
    /// struct My {
    ///   value: i32,
    ///   next: Box<My>,
    /// }
    /// ```
    ///
    /// Requires that the implementor decide whether the value of the `value` field should affect
    /// equivalence.  Either way is supported.  The implementor could decide to always return
    /// `true` to ignore the field and allow the algorithm to just compare the descendent, or the
    /// implementor could make the result correspond to whether the values of the field are the
    /// same or not.
    ///
    /// Or, e.g. a node type like:
    ///
    /// ```rust
    /// enum My {
    ///   A(Box<My>, Box<My>),
    ///   B(Box<My>, Box<My>),
    /// }
    /// ```
    ///
    /// Requires that the implementor decide whether the difference between the `A` and `B`
    /// variants should affect equivalence.  Either way is supported.  Since both variants have
    /// the same amount of edges (assuming [`Self::amount_edges`] is implemented like that), the
    /// implementor could decide to always return `true` to ignore differences in the variants and
    /// allow the algorithm to just compare the descendents, or the implementor could make the
    /// result correspond to whether the variants are the same or not.
    ///
    /// Or, e.g. a node type like:
    ///
    /// ```rust
    /// enum My {
    ///   A,
    ///   B(Box<My>),
    /// }
    /// ```
    ///
    /// It is sufficient to always return `true`, when [`Self::amount_edges`] returns `0.into()`
    /// for the `A` variant or `1.into()` for the `B` variant, because this is used by
    /// [`Self::equiv_modulo_descendents_then_amount_edges`] and the algorithm will detect the
    /// unequivalence that way instead.
    fn equiv_modulo_edges(
        &self,
        other: &Self,
    ) -> bool;

    /// Check if the nodes are equivalent in their own directly-contained semantically-significant
    /// values ignoring their descendent nodes and check if their amounts of edges are similar
    /// enough that their descendents will need to be checked for equivalence.  If both conditions
    /// are true, return the amount of edges that the algorithm should descend, else return
    /// `None`.
    ///
    /// The implementor must use [`Self::equiv_modulo_edges`] and [`Self::amount_edges`] to check
    /// the conditions, but may do so in any order.  This allows the implementation to optimize
    /// the order to be the most efficient for its type.
    ///
    /// The implementor must ensure that a `Some(result)` upholds: `self.amount_edges() >= result
    /// && other.amount_edges() >= result`, so that there are enough descendents of each to
    /// descend into.
    ///
    /// The default implementation checks that `self.amount_edges() == other.amount_edges()` and
    /// `self.equiv_modulo_edges(other)`, in that order, and, when true, returns the amount of
    /// edges.  This is intended for types where [`Self::amount_edges`] is cheaper than
    /// [`Self::equiv_modulo_edges`] and so should be checked first, and where the nodes should be
    /// considered unequivalent if their amounts of edges are not the same, and where all the
    /// edges should be descended.  For types that do not want all of those aspects, a custom
    /// implementation will need to be provided, and it must fulfill all the above requirements.
    #[inline]
    fn equiv_modulo_descendents_then_amount_edges(
        &self,
        other: &Self,
    ) -> Option<Self::Index>
    {
        let (az, bz) = (self.amount_edges(), other.amount_edges());
        (az == bz && self.equiv_modulo_edges(other)).then(|| az)
    }
}
