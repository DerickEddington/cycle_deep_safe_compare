// TODO(#9): Reorganize into sub modules

use std::{
    cell::Cell,
    collections::HashMap,
    hash::Hash,
    marker::PhantomData,
    ops::AddAssign,
    ptr,
    rc::Rc,
};


// TODO: These values are from the paper, which is for Scheme.  Other values
// might be more optimal for this Rust variation?
const PRE_LIMIT: u16 = 400;
const FAST_LIMIT: u16 = 2 * PRE_LIMIT;
#[allow(clippy::integer_division)]
const SLOW_LIMIT: u16 = PRE_LIMIT / 10;
#[allow(clippy::as_conversions)]
const SLOW_LIMIT_NEG: i32 = -(SLOW_LIMIT as i32);


/// What the main equivalence algorithm needs from a type.
pub trait Node
{
    /// Determines when nodes are the same identical node and so can immediately
    /// be considered equivalent without checking their values, edges, nor
    /// descendents.  The size of and methods on this type should be small and
    /// very cheap.
    ///
    /// For types where only nodes that are the same object in memory can be
    /// considered identical, pointer/address equality and hashing should be
    /// used by defining this type to be `*const T` where `T` is either `Self`
    /// or the primary inner type.  Such pointers are never dereferenced, and so
    /// there is no `unsafe` usage.  (Unfortunately, trying to use `&T` would
    /// cause too many difficulties with lifetimes.  Using `*const T` is valid
    /// for the algorithm because the lifetimes of the `&Self` borrows for the
    /// entry-point [`precheck_interleave_equiv`] function calls outlive such
    /// pointers used internally, and so the `Self` objects cannot move during
    /// those lifetimes and so the pointers remain valid.)
    ///
    /// For other types where different `Self` objects can represent the same
    /// identical node, some approach following that should be provided, and the
    /// pointer/address approach should not be used.
    type Id: Eq + Hash + Clone;

    /// Determines what is used to index descendent nodes and to represent the
    /// amount of them.  The primitive unsigned integer types, like `usize`, are
    /// a common choice, but it may be anything that satisfies the trait bounds.
    ///
    /// Only `Self::Index::from(0)`, `Self::Index::from(1)`, and
    /// `Self::Index::add_assign(index, 1.into())` are actually used by the
    /// algorithm, and so the type does not actually have to support `From<u8>`
    /// for the rest of the `u8` range, and does not actually have to support
    /// `AddAssign` of increments other than the unit value nor of results
    /// beyond the maximum possible amount of edges.
    ///
    /// E.g. for graphs with nodes whose amounts of edges are always smaller
    /// than some limit, it might be desirable, for efficiency, to use an index
    /// type smaller than `usize`.  Or for other node types, it might be more
    /// logical or convenient to use an index type that is not a number.
    type Index: Eq + Ord + AddAssign + From<u8>;

    /// The descendent node type may be the same as `Self` or may be different
    /// as long as the [`Id`](Node::Id) type is the same.
    type Edge: Node<Id = Self::Id>;

    /// Get the identity of the `self` node.  The result must only be `==` to
    /// another node's when the nodes should be considered identical.
    fn id(&self) -> Self::Id;

    /// Determines how many edges the `self` node has that the algorithm will
    /// descend into and check.  All indices in the range `0.into()
    /// .. self.amount_edges()` must be valid to call
    /// [`self.get_edge(index)`](Self::get_edge) with.
    fn amount_edges(&self) -> Self::Index;

    /// Get descendent node by index.  The index must be within the range
    /// `0.into() .. self.amount_edges()`.  The algorithm calls this method, for
    /// each index in that range, to descend into each edge.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.  But since the same implementor
    /// controls [`Self::amount_edges`], and when that is implemented correctly,
    /// as it must be, then such out-of-bounds panics are impossible, as used by
    /// the algorithm.
    fn get_edge(
        &self,
        index: &Self::Index,
    ) -> Self::Edge;

    /// Check if the nodes are equivalent in their own directly-contained
    /// semantically-significant values ignoring their edges and ignoring their
    /// descendent nodes.  This is intended to be used by
    /// [`Self::equiv_modulo_descendents_then_amount_edges`].
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
    /// Requires that the implementor decide whether the value of the `value`
    /// field should affect equivalence.  Either way is supported.  The
    /// implementor could decide to always return `true` to ignore the field and
    /// allow the algorithm to just compare the descendent, or the implementor
    /// could make the result correspond to whether the values of the field are
    /// the same or not.
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
    /// Requires that the implementor decide whether the difference between the
    /// `A` and `B` variants should affect equivalence.  Either way is
    /// supported.  Since both variants have the same amount of edges (assuming
    /// [`Self::amount_edges`] is implemented like that), the implementor could
    /// decide to always return `true` to ignore differences in the variants and
    /// allow the algorithm to just compare the descendents, or the implementor
    /// could make the result correspond to whether the variants are the same or
    /// not.
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
    /// It is sufficient to always return `true`, when [`Self::amount_edges`]
    /// returns `0.into()` for the `A` variant or `1.into()` for the `B`
    /// variant, because this is used by
    /// [`Self::equiv_modulo_descendents_then_amount_edges`] and the algorithm
    /// will detect the unequivalence that way instead.
    fn equiv_modulo_edges(
        &self,
        other: &Self,
    ) -> bool;

    /// Check if the nodes are equivalent in their own directly-contained
    /// semantically-significant values ignoring their descendent nodes and
    /// check if their amounts of edges are similar enough that their
    /// descendents will need to be checked for equivalence.  If both conditions
    /// are true, return the amount of edges that the algorithm should descend,
    /// else return `None`.
    ///
    /// The implementor must use [`Self::equiv_modulo_edges`] and
    /// [`Self::amount_edges`] to check the conditions, but may do so in any
    /// order.  This allows the implementation to optimize the order to be the
    /// most efficient for its type.
    ///
    /// The implementor must ensure that a `Some(result)` upholds:
    /// `self.amount_edges() >= result && other.amount_edges() >= result`, so
    /// that there are enough descendents of each to descend into.
    ///
    /// The default implementation checks that `self.amount_edges() ==
    /// other.amount_edges()` and `self.equiv_modulo_edges(other)`, in that
    /// order, and, when true, returns the amount of edges.  This is intended
    /// for types where [`Self::amount_edges`] is cheaper than
    /// [`Self::equiv_modulo_edges`] and so should be checked first, and where
    /// the nodes should be considered unequivalent if their amounts of edges
    /// are not the same, and where all the edges should be descended.  For
    /// types that do not want all of those aspects, a custom implementation
    /// will need to be provided, and it must fulfill all the above
    /// requirements.
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


/// The main equivalence algorithm which can be used for [`PartialEq`]
/// implementations.  TODO(#10): more about...
pub fn precheck_interleave_equiv<N: Node + ?Sized>(
    a: &N,
    b: &N,
) -> bool
{
    match precheck(a, b, PRE_LIMIT.into()) {
        EquivResult::Equiv => true,
        EquivResult::Unequiv => false,
        EquivResult::Abort => interleave(a, b, -1),
    }
}


fn precheck<N: Node + ?Sized>(
    a: &N,
    b: &N,
    limit: i32,
) -> EquivResult
{
    struct Precheck<I>(PhantomData<I>);

    impl<I> EquivControl for Equiv<Precheck<I>>
    {
        type Id = I;

        fn do_descend<N: Node<Id = Self::Id> + ?Sized>(
            &mut self,
            _a: &N,
            _b: &N,
        ) -> DoDescend
        {
            if self.limit > 0 { DoDescend::Yes } else { DoDescend::NoAbort }
        }
    }

    let mut e = Equiv { limit, state: Precheck(PhantomData) };
    e.equiv(a, b)
}


enum DoDescend
{
    Yes,
    NoContinue,
    NoAbort,
}

enum EquivResult
{
    Unequiv,
    Abort,
    Equiv,
}

trait EquivControl
{
    type Id;

    fn do_descend<N: Node<Id = Self::Id> + ?Sized>(
        &mut self,
        a: &N,
        b: &N,
    ) -> DoDescend;
}

struct Equiv<S>
{
    limit: i32,
    state: S,
}

impl<S, I: Eq> Equiv<S>
where Self: EquivControl<Id = I>
{
    fn equiv<N: Node<Id = I> + ?Sized>(
        &mut self,
        a: &N,
        b: &N,
    ) -> EquivResult
    {
        use {
            DoDescend::{
                NoAbort,
                NoContinue,
                Yes,
            },
            EquivResult::{
                Abort,
                Equiv,
                Unequiv,
            },
        };

        if a.id() == b.id() {
            Equiv
        }
        else if let Some(amount_edges) = a.equiv_modulo_descendents_then_amount_edges(b) {
            let mut i = 0.into();
            if i < amount_edges {
                match self.do_descend(a, b) {
                    Yes =>
                        while i < amount_edges {
                            let (ae, be) = (a.get_edge(&i), b.get_edge(&i));
                            self.limit = self.limit.saturating_sub(1);
                            match self.equiv(&ae, &be) {
                                Equiv => (),
                                result @ (Unequiv | Abort) => return result,
                            }
                            i += 1.into();
                        },
                    NoContinue => (),
                    NoAbort => return Abort,
                }
            }
            Equiv
        }
        else {
            Unequiv
        }
    }
}


fn interleave<N: Node + ?Sized>(
    a: &N,
    b: &N,
    limit: i32,
) -> bool
{
    impl<I: Eq + Hash + Clone> EquivControl for Equiv<EquivClasses<I>>
    {
        type Id = I;

        fn do_descend<N: Node<Id = Self::Id> + ?Sized>(
            &mut self,
            a: &N,
            b: &N,
        ) -> DoDescend
        {
            fn rand_limit(max: u16) -> i32
            {
                fastrand::i32(0 ..= max.into())
            }

            match self.limit {
                // "fast" phase
                0 .. => DoDescend::Yes,
                // "slow" phase
                SLOW_LIMIT_NEG ..= -1 =>
                    if self.state.same_class(&a.id(), &b.id()) {
                        // This is what prevents traversing descendents that
                        // have already been checked, which prevents infinite
                        // loops on cycles and is more efficient on shared
                        // structure.  Reset the counter so that "slow" will be
                        // used for longer, on the theory that further
                        // equivalences in descendents are more likely since we
                        // found an equivalence (which is critical for avoiding
                        // stack overflow with shapes like "degenerate
                        // cyclic".).
                        self.limit = 0;
                        DoDescend::NoContinue
                    }
                    else {
                        DoDescend::Yes
                    },
                // "slow" limit reached, change to "fast" phase
                _ /* MIN .. SLOW_LIMIT_NEG */ => {
                    // Random limits for "fast" "reduce the likelihood of
                    // repeatedly tripping on worst-case behavior in cases where
                    // the sizes of the input graphs happen to be related to the
                    // chosen bounds in a bad way".
                    self.limit = rand_limit(FAST_LIMIT);
                    DoDescend::Yes
                },
            }
        }
    }

    let mut e = Equiv { limit, state: EquivClasses::new() };
    matches!(e.equiv(a, b), EquivResult::Equiv)
}


struct EquivClasses<Key>
{
    map: HashMap<Key, Rc<Cell<EquivClassChain>>>,
}

#[derive(Clone)]
enum EquivClassChain
{
    End(Rc<Cell<Representative>>),
    Next(Rc<Cell<Self>>),
}

#[derive(Copy, Clone, Debug)]
struct Representative
{
    weight: usize,
}

impl Representative
{
    fn default() -> Rc<Cell<Self>>
    {
        Rc::new(Cell::new(Self { weight: 1 }))
    }
}

impl PartialEq for Representative
{
    fn eq(
        &self,
        other: &Self,
    ) -> bool
    {
        ptr::eq(self, other)
    }
}
impl Eq for Representative {}

impl EquivClassChain
{
    fn default() -> Rc<Cell<Self>>
    {
        Self::new(Representative::default())
    }

    fn new(rep: Rc<Cell<Representative>>) -> Rc<Cell<Self>>
    {
        Rc::new(Cell::new(Self::End(rep)))
    }

    /// This type uses `Cell`, instead of `RefCell`, so that panics are
    /// impossible, which requires the approach of this function because our
    /// type cannot be `Copy`.
    fn clone_inner(it: &Rc<Cell<Self>>) -> Self
    {
        let dummy = Self::Next(Rc::clone(it));
        let inner = it.replace(dummy);
        let result = inner.clone();
        it.set(inner);
        result
    }

    fn rep_of(it: &Rc<Cell<Self>>) -> Rc<Cell<Representative>>
    {
        let it_inner = Self::clone_inner(it);
        match it_inner {
            Self::End(rep) => rep,

            Self::Next(mut next) => {
                let mut cur = Rc::clone(it);
                loop {
                    let next_inner = Self::clone_inner(&next);
                    match next_inner {
                        Self::End(rep) => break rep,

                        Self::Next(next_next) => {
                            cur.set(Self::Next(Rc::clone(&next_next)));
                            cur = next;
                            next = next_next;
                        },
                    }
                }
            },
        }
    }
}

impl<K: Eq + Hash + Clone> EquivClasses<K>
{
    fn new() -> Self
    {
        Self { map: HashMap::new() }
    }

    fn none_seen(
        &mut self,
        ak: &K,
        bk: &K,
    )
    {
        let aec = EquivClassChain::default();
        let bec = Rc::clone(&aec);
        let _ignored1 = self.map.insert(ak.clone(), aec);
        let _ignored2 = self.map.insert(bk.clone(), bec);
    }

    fn some_seen(
        &mut self,
        oec: &Rc<Cell<EquivClassChain>>,
        k: &K,
    )
    {
        let rep = EquivClassChain::rep_of(oec);
        let ec = EquivClassChain::new(rep);
        let _ignored = self.map.insert(k.clone(), ec);
    }

    fn all_seen(
        aec: &Rc<Cell<EquivClassChain>>,
        bec: &Rc<Cell<EquivClassChain>>,
    ) -> bool
    {
        let arep = EquivClassChain::rep_of(aec);
        let brep = EquivClassChain::rep_of(bec);

        if arep == brep {
            true
        }
        else {
            let (aw, bw) = (arep.get().weight, brep.get().weight);
            let (larger_chain, larger_rep, smaller_chain);

            if aw >= bw {
                larger_chain = aec;
                larger_rep = arep;
                smaller_chain = bec;
            }
            else {
                larger_chain = bec;
                larger_rep = brep;
                smaller_chain = aec;
            }
            smaller_chain.set(EquivClassChain::Next(Rc::clone(larger_chain)));
            larger_rep.set(Representative { weight: aw.saturating_add(bw) });
            false
        }
    }

    fn same_class(
        &mut self,
        ak: &K,
        bk: &K,
    ) -> bool
    {
        match (self.map.get(ak), self.map.get(bk)) {
            (None, None) => {
                self.none_seen(ak, bk);
                false
            },
            (Some(aec), None) => {
                let aec = &Rc::clone(aec); // To end borrow of `self`.
                self.some_seen(aec, bk);
                false
            },
            (None, Some(bec)) => {
                let bec = &Rc::clone(bec); // To end borrow of `self`.
                self.some_seen(bec, ak);
                false
            },
            (Some(aec), Some(bec)) => Self::all_seen(aec, bec),
        }
    }
}


#[cfg(test)]
mod tests
{
    use super::*;

    enum Datum
    {
        Leaf,
        Pair(Box<Self>, Box<Self>),
    }

    fn leaf() -> Datum
    {
        Datum::Leaf
    }

    fn pair(
        a: Datum,
        b: Datum,
    ) -> Datum
    {
        Datum::Pair(Box::new(a), Box::new(b))
    }

    fn end_pair() -> Datum
    {
        pair(leaf(), leaf())
    }

    impl Node for &Datum
    {
        type Edge = Self;
        type Id = *const Datum;
        type Index = u8;

        fn id(&self) -> Self::Id
        {
            *self
        }

        fn amount_edges(&self) -> Self::Index
        {
            match self {
                Datum::Leaf => 0,
                Datum::Pair(_, _) => 2,
            }
        }

        fn get_edge(
            &self,
            idx: &Self::Index,
        ) -> Self::Edge
        {
            match (idx, self) {
                #![allow(clippy::panic)]
                (0, Datum::Pair(a, _)) => a,
                (1, Datum::Pair(_, b)) => b,
                _ => panic!("invalid"),
            }
        }

        fn equiv_modulo_edges(
            &self,
            _other: &Self,
        ) -> bool
        {
            true
        }
    }

    #[test]
    fn limiting()
    {
        #[derive(PartialEq, Eq, Debug)]
        enum Result
        {
            Unequiv(i32),
            Abort(i32),
            Equiv(i32),
        }

        fn eqv(
            a: &Datum,
            b: &Datum,
            limit: i32,
        ) -> Result
        {
            struct State<I>(PhantomData<I>);

            impl<I> EquivControl for Equiv<State<I>>
            {
                type Id = I;

                fn do_descend<N: Node<Id = Self::Id> + ?Sized>(
                    &mut self,
                    _a: &N,
                    _b: &N,
                ) -> DoDescend
                {
                    if self.limit > 0 { DoDescend::Yes } else { DoDescend::NoAbort }
                }
            }

            let mut e = Equiv { limit, state: State(PhantomData) };
            match e.equiv(&a, &b) {
                EquivResult::Abort => Result::Abort(e.limit),
                EquivResult::Unequiv => Result::Unequiv(e.limit),
                EquivResult::Equiv => Result::Equiv(e.limit),
            }
        }

        assert_eq!(eqv(&leaf(), &leaf(), 42), Result::Equiv(42));
        assert_eq!(eqv(&leaf(), &leaf(), -1), Result::Equiv(-1));
        assert_eq!(eqv(&leaf(), &end_pair(), 42), Result::Unequiv(42));
        assert_eq!(eqv(&end_pair(), &leaf(), 42), Result::Unequiv(42));
        assert_eq!(eqv(&end_pair(), &end_pair(), 7), Result::Equiv(5));
        assert_eq!(eqv(&pair(leaf(), end_pair()), &pair(leaf(), end_pair()), 7), Result::Equiv(3));
        assert_eq!(eqv(&end_pair(), &end_pair(), 0), Result::Abort(0));
        assert_eq!(eqv(&pair(leaf(), end_pair()), &pair(leaf(), end_pair()), 1), Result::Abort(-1));
        assert_eq!(eqv(&pair(leaf(), leaf()), &pair(leaf(), end_pair()), 42), Result::Unequiv(40));
        assert_eq!(
            {
                let x = pair(end_pair(), leaf());
                eqv(&x, &x, 0)
            },
            Result::Equiv(0)
        );
    }

    #[test]
    fn representative()
    {
        {
            #![allow(clippy::eq_op)]
            let r = &Representative { weight: 0 };
            assert!(r == r);
        };
        {
            let r1 = &Representative { weight: 1 };
            let r2 = r1;
            assert_eq!(r1, r2);
        };
        {
            let r1 = Rc::new(Representative { weight: 2 });
            let r2 = Rc::clone(&r1);
            assert_eq!(r1, r2);
        };
        {
            let r1 = Rc::new(Representative { weight: 3 });
            let r2 = Rc::new(Representative { weight: 3 });
            assert_ne!(r1, r2);
        };
    }

    #[test]
    fn rep_of()
    {
        let rep1 = Representative::default();
        let ecc1 = EquivClassChain::new(Rc::clone(&rep1));
        let ecc2 = EquivClassChain::new(Rc::clone(&rep1));
        let ecc3 = Rc::new(Cell::new(EquivClassChain::Next(Rc::clone(&ecc1))));
        let ecc4 = Rc::new(Cell::new(EquivClassChain::Next(Rc::clone(&ecc2))));
        let ecc5 = Rc::new(Cell::new(EquivClassChain::Next(Rc::clone(&ecc4))));
        let ecc6 = Rc::new(Cell::new(EquivClassChain::Next(Rc::clone(&ecc5))));

        assert_eq!(EquivClassChain::rep_of(&ecc1), rep1);
        assert_eq!(EquivClassChain::rep_of(&ecc2), rep1);
        assert_eq!(EquivClassChain::rep_of(&ecc3), rep1);
        assert_eq!(EquivClassChain::rep_of(&ecc4), rep1);
        assert_eq!(EquivClassChain::rep_of(&ecc5), rep1);
        assert_eq!(EquivClassChain::rep_of(&ecc6), rep1);

        let rep2 = Representative::default();
        let ecc7 = EquivClassChain::new(Rc::clone(&rep2));
        assert_ne!(EquivClassChain::rep_of(&ecc7), rep1);
    }

    #[test]
    fn same_class()
    {
        let mut ec = EquivClasses::new();
        let keys = ['a', 'b', 'c', 'd', 'e', 'f'];

        assert!(!ec.same_class(&keys[0], &keys[1]));
        assert!(ec.same_class(&keys[0], &keys[1]));

        assert!(!ec.same_class(&keys[0], &keys[2]));
        assert!(ec.same_class(&keys[0], &keys[2]));
        assert!(ec.same_class(&keys[1], &keys[2]));

        assert!(!ec.same_class(&keys[3], &keys[2]));
        assert!(ec.same_class(&keys[3], &keys[2]));
        assert!(ec.same_class(&keys[3], &keys[1]));
        assert!(ec.same_class(&keys[3], &keys[0]));

        assert!(!ec.same_class(&keys[4], &keys[5]));
        assert!(ec.same_class(&keys[4], &keys[5]));
        assert!(!ec.same_class(&keys[1], &keys[4]));

        for a in &keys {
            for b in &keys {
                assert!(ec.same_class(a, b));
            }
        }
    }
}
