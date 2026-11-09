//! The n-ary tree of panes: what is beside what, and the invariants a move has to keep.
//!
//! **Why n-ary and not binary**, which is the whole reason this file exists: with a binary tree,
//! three consoles in a row get nested and the outer divider moves **a block of two** — the thing
//! `tmux` is most criticised for. Splitting in the same orientation as the branch that already holds
//! the leaf inserts a **sibling**, and that is what keeps every divider moving only its two
//! neighbours. **D14**.
//!
//! The arithmetic of sharing an area out is [`crate::tiling`]'s; this file decides *what* is shared.
//!
//! **And it is where a description becomes processes**: [`Node::materialize`] is the one path in the
//! program that turns what was read from disk into open consoles, one per leaf. It is here because
//! the shape it walks is this file's, and it takes an already-validated document precisely so that
//! this path cannot fork a shell for a tree that should never have existed.
//! Its mirror to and from what is stored on disk lives at the bottom, because the shape written down
//! is the tree's own and nobody else's.
//!
//! It came out of `lib.rs` under **D28**.

use crate::tiling::{
    DIVIDER_THICKNESS, Divider, Orientation, Side, Tiling, Weight, tile, weights_for, weights_of,
};
use crate::{OpeningLayout, Pane, panel, session};
use eframe::egui;
use std::time::SystemTime;

/// The tree a layout produces: **the columns on the outside and the rows on the inside**.
///
/// The order of the leaves is the order they are painted in and the order the window opened them
/// in, so with `2x2` they are the top and bottom ones of the left column, and then the two on the
/// right. The report's names hang off that order.
///
/// **A branch with a single child is never built**, which is the same invariant closing maintains
/// by collapsing: with one column what is inside is returned directly, and with a single console,
/// its leaf. **How many columns come out is counted from the consoles there are, not from what the
/// layout asked for.** They are two sources for the same datum, and with two you can take the wrong
/// one: asking for four columns and delivering two consoles would leave **two branches with no
/// child at all**, whose sum of weights is zero — the `NaN` in a `Rect` that the [`Weight`] type
/// exists to make impossible, sneaked in through another door.
fn in_layout(panes: Vec<Pane>, rows: usize) -> Node {
    // `Pane` is not `Clone` —it carries a pseudoterminal inside—, so the leaves are shared out by
    // consuming the vector instead of slicing it.
    let how_many_columns = panes.len().div_ceil(rows);
    let mut remaining = panes.into_iter();
    let columns: Vec<Node> = (0..how_many_columns)
        .map(|_| {
            let leaves = remaining.by_ref().take(rows).map(Node::Leaf).collect();
            in_branch(Orientation::Rows, leaves)
        })
        .collect();

    in_branch(Orientation::Columns, columns)
}

/// A branch with those children in equal parts, or the loose child if it is a single one.
///
/// **It delegates to [`in_branch_with_weights`] instead of repeating its body**, and a review asked
/// for that: both had the same line collapsing the branch with a single child, which is **the
/// invariant that makes each divider move only its two neighbors**. Two copies of an invariant are
/// one too many — the day that rule changes, one is fixed and the other is left behind.
fn in_branch(orientation: Orientation, children: Vec<Node>) -> Node {
    in_branch_with_weights(
        orientation,
        weights_for(children.len())
            .into_iter()
            .zip(children)
            .collect(),
    )
}

/// A branch with those children and **those weights**, or the loose child if it is a single one.
///
/// **The sibling of [`in_branch`], which tiles in equal parts.** It is needed since a layout can
/// come from a file: the weights of a tree the user left placed **are not the factory ones**, and
/// rebuilding it with the factory ones would give back a window that looks like theirs without
/// being it — the worst possible result, because it is not noticeable that it has failed.
///
/// It collapses the branch with a single child for the same reason as the other one: it is the
/// invariant that makes each divider move only its two neighbors.
fn in_branch_with_weights(orientation: Orientation, children: Vec<(Weight, Node)>) -> Node {
    let mut children = children;
    if children.len() == 1 {
        return children.remove(0).1;
    }

    Node::Branch {
        orientation,
        children: children
            .into_iter()
            .map(|(weight, node)| Child { weight, node })
            .collect(),
    }
}

/// How a tab's consoles are tiled: a leaf is a console, a branch is a cut.
///
/// **A tree and not a list of rectangles**, which is the other way of describing a layout. The tree
/// guarantees **by construction** that what is painted covers the area with no gaps and no
/// overlaps: every branch tiles what it was given among its children, and there is no way to write
/// a state where there is space to spare or missing. With loose rectangles that invariant becomes
/// code to maintain, and dragging a pane elsewhere —the next milestone— turns into a packing
/// problem.
///
/// **N-ary and not binary**, which is how `tmux` and company do it. With the binary one, three
/// consoles in a row force two branches to be nested, and then the outer divider moves **a block of
/// two** instead of its neighbor — the behavior whoever uses those terminals complains about most.
/// Here the three are siblings of a single branch and each divider moves only the two it separates.
/// **No `Box` in the leaf, and it is a decision with its arithmetic done.** A console takes some 3
/// KiB, so every node of the tree takes that even if it is a branch: with eight panes that is
/// fifteen nodes, some **45 KiB per tab**, and with eight tabs 360 KiB. Beside the queue ceiling
/// every pane already has —1 MiB, that is 8 MiB with those same eight— it is noise. What a `Box`
/// would cost in exchange is a pointer hop per leaf **on the painting path**, which is precisely
/// the one this project measures and the one the decision about the UI framework hangs off.
#[allow(
    clippy::large_enum_variant,
    reason = "45 KiB per tab against a pointer hop on the path that is measured"
)]
pub(crate) enum Node {
    /// A console, and the end of a branch.
    Leaf(Pane),
    /// A cut: its children tile the area in the direction the orientation says.
    Branch {
        orientation: Orientation,
        children: Vec<Child>,
    },
}

/// A child of a branch, with what the tiling gives it.
pub(crate) struct Child {
    pub(crate) weight: Weight,
    pub(crate) node: Node,
}

impl Node {
    /// This node's consoles, from the first to the last in the order in which they are painted.
    ///
    /// **It returns a vector and not a lazy iterator**, and it is a decision with a price: walking
    /// a tree without allocating demands either recursion with a closure —which does not compose
    /// with the rest of the code, which expects an iterator— or a stack of its own, which allocates
    /// too. The vector costs one allocation per call, with two consoles it is two pointers, and
    /// **what it costs is declared**: the painting walk is timed and subtracted, for the same
    /// reason the `fork`s of opening a tab are timed.
    pub(crate) fn leaves(&self) -> Vec<&Pane> {
        let mut leaves = Vec::new();
        self.walk(&mut |pane| leaves.push(pane));
        leaves
    }

    pub(crate) fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Pane)) {
        match self {
            Self::Leaf(pane) => visit(pane),
            Self::Branch { children, .. } => {
                for child in children {
                    child.node.walk(visit);
                }
            }
        }
    }

    fn leaves_mut(&mut self) -> Vec<&mut Pane> {
        let mut leaves = Vec::new();
        self.walk_mut(&mut |pane| leaves.push(pane));
        leaves
    }

    fn walk_mut<'a>(&'a mut self, visit: &mut impl FnMut(&'a mut Pane)) {
        match self {
            Self::Leaf(pane) => visit(pane),
            Self::Branch { children, .. } => {
                for child in children {
                    child.node.walk_mut(visit);
                }
            }
        }
    }

    /// Where each console falls inside the area, and where each divider.
    ///
    /// **The leaves come out in the same order as [`Node::leaves`]**, and that is what allows them
    /// to be matched by position instead of dragging the geometry inside the tree. It is the same
    /// deal the tiling of two already had with its two rectangles.
    pub(crate) fn place(&self, area: egui::Rect, minimum: f32) -> Tiling {
        match self {
            Self::Leaf(_) => Tiling {
                parts: vec![area],
                dividers: Vec::new(),
            },
            Self::Branch {
                orientation,
                children,
            } => {
                let weights: Vec<Weight> = children.iter().map(|child| child.weight).collect();
                let mine = tile(area, *orientation, &weights, minimum);

                let mut parts = Vec::with_capacity(children.len());
                let mut dividers = mine.dividers;
                for (child, part) in children.iter().zip(mine.parts) {
                    let inside = child.node.place(part, minimum);
                    parts.extend(inside.parts);
                    dividers.extend(inside.dividers);
                }

                Tiling { parts, dividers }
            }
        }
    }

    /// Swaps two consoles around, **without touching the shape of the tree**.
    ///
    /// **The weights stay with the place, not with the console.** What is asked for on dropping one
    /// on top of another is that each takes the other's slot; carrying its size along as well, the
    /// neighbors of both would move without anybody having touched a single divider of theirs.
    ///
    /// **With the same name twice it does nothing, and that is said here and not in the caller**:
    /// dropping a console on itself is a one-second gesture, so it is an everyday case and not an
    /// abuse, and leaving it out would force every caller to remember.
    ///
    /// Returns whether the swap happened.
    ///
    /// **Only the tests use it, and that is why it carries `cfg(test)`.** It was born as the
    /// gesture's effect back when dropping one console on another swapped them; since dropping
    /// **relocates**, production does not call it. It survives because it is the cheapest way to
    /// build a tree where each console's **name** does not match its **place**, which is the only
    /// thing that tells reading one from reading the other — the disguise this project has on file.
    #[cfg(test)]
    pub(crate) fn swap_panes(&mut self, one: usize, other: usize) -> bool {
        if one == other {
            return false;
        }

        let mut leaves = self.leaves_mut();
        let (Some(here), Some(there)) = (
            leaves.iter().position(|leaf| leaf.name == one),
            leaves.iter().position(|leaf| leaf.name == other),
        ) else {
            return false;
        };

        // **`split_at_mut` is what allows both to be held at once.** Borrowing two elements of the
        // same list separately is not allowed by the borrow checker, and the easy way out —cloning
        // one to copy it into the other— does not exist: a console carries its pseudoterminal and
        // its program inside.
        let (before, after) = (here.min(there), here.max(there));
        let (head, tail) = leaves.split_at_mut(after);
        std::mem::swap(&mut *head[before], &mut *tail[0]);
        true
    }

    /// Puts `new` next to the leaf named `name`, cutting in that direction.
    ///
    /// **Returns the pane it has not placed**, which is the way of saying «that leaf is not around
    /// here» without leaving it along the way: a console cannot be cloned to try its luck in every
    /// branch, and dropping it would kill its program. The caller gets theirs back and goes on
    /// looking.
    ///
    /// **Splitting in the same direction as the branch that already contains it inserts a sibling,
    /// not a new branch.** It is what avoids the binary tree's nesting: with three consoles in a
    /// row, nesting would make the outer divider move a block of two, which is the `tmux` behavior
    /// whoever uses it complains about most. As siblings, each divider moves only its two
    /// neighbors.
    ///
    /// **And the two share out the weight of the one that was split**, instead of giving the new
    /// one a weight of its own: that way the siblings that were already there do not move a single
    /// pixel, which is the same thing dragging a divider promises.
    pub(crate) fn split(&mut self, name: usize, side: Side, new: Pane) -> Option<Pane> {
        let cut_along = side.cut_along();
        match self {
            Self::Leaf(pane) => {
                if pane.name != name {
                    return Some(new);
                }

                self.make_a_branch(side, new);
                None
            }
            Self::Branch {
                orientation,
                children,
            } => {
                // **The leaf is looked for here only for the sibling case**, which is the only one
                // that needs the whole list: inserting cannot be done while walking. When the cut
                // goes in the other direction there is no need to look at anything — the leaf turns
                // into a branch, and the descent further down takes care of that on reaching it.
                // Looking for it here as well would duplicate in two places the decision about
                // **where the new console appears**, and a test covering one would leave the other
                // uncovered.
                let sibling = (*orientation == cut_along)
                    .then(|| {
                        children
                            .iter()
                            .position(|child| child.node.is_the_pane_named(name))
                    })
                    .flatten();

                if let Some(at) = sibling {
                    // Half of a positive, finite weight still is one **as long as the starting
                    // weight is not tiny**, and the tiny ones are reached by dragging: see
                    // [`Weight::scaled`], which is who sets the floor. The geometric floor of
                    // [`can_split`] bounds what is seen, not the number.
                    let half = Weight::scaled(children[at].weight.value() / 2.0);
                    children[at].weight = half;
                    // **The side decides which part of the slot the new one comes out of**, and
                    // only that: the siblings at both ends do not notice, because what is shared
                    // out is the weight of a single one.
                    let slot = if side.before() { at } else { at + 1 };
                    children.insert(
                        slot,
                        Child {
                            weight: half,
                            node: Self::Leaf(new),
                        },
                    );

                    return None;
                }

                // It was not among this branch's leaves: it goes on inside its children, carrying
                // the pane from branch to branch until one of them places it.
                let mut traveler = new;
                for child in children {
                    // The `?` says just what is needed: if the child does **not** give it back it
                    // is because it placed it, and then there is nothing left to go on looking for
                    // here.
                    traveler = child.node.split(name, side, traveler)?;
                }

                Some(traveler)
            }
        }
    }

    /// Turns this leaf into a branch of two: the one that was there, and the new one on the side
    /// asked for.
    ///
    /// The two weights are equal because a pane that is born by splitting is born **halved**, and
    /// the weight of the whole branch —the one its outer siblings see— is the business of whoever
    /// contains it, not of here.
    fn make_a_branch(&mut self, side: Side, new: Pane) {
        let cut_along = side.cut_along();
        let weight = Weight::new(1.0).expect("one is a valid weight");
        // The marker left in the slot is a branch with no children: **something** is needed there
        // in order to take the console out of the leaf without cloning it, and an empty branch is
        // the cheapest thing that can be put in. It lives as long as the next line takes to replace
        // it with the real branch, so no walk gets to see it.
        let old = std::mem::replace(
            self,
            Self::Branch {
                orientation: cut_along,
                children: Vec::new(),
            },
        );

        let arriving = Child {
            weight,
            node: Self::Leaf(new),
        };
        let existing = Child { weight, node: old };

        *self = Self::Branch {
            orientation: cut_along,
            children: if side.before() {
                vec![arriving, existing]
            } else {
                vec![existing, arriving]
            },
        };
    }

    /// Whether somewhere down here there is a console with that name.
    fn has_leaf(&self, name: usize) -> bool {
        let mut found = false;
        self.walk(&mut |pane| found |= pane.name == name);
        found
    }

    /// Takes a console out of wherever it is and puts it on the side asked for of another.
    ///
    /// **It takes the tree by value and returns a whole one, whether it goes well or badly.** That
    /// is the way it is guaranteed that a movement happens whole or does not happen, and not a
    /// promise that has to be remembered: as long as the transformation does not reach the end, the
    /// caller **has no tree to put back**, so there is no intermediate state in which a console
    /// —with its program alive inside— has been taken out of one place and is not yet in another.
    /// When it is rejected, the tree comes back just as it was.
    ///
    /// **The checks go before anything is touched, and they are the three that can really fail:**
    /// moving a console onto itself, a name that no longer exists —the destination could have been
    /// closed between grabbing and dropping— and a tab with a single console, which has nowhere to
    /// move anything to.
    ///
    /// **After those three, inserting cannot fail, and that can be demonstrated instead of
    /// trusted**: the destination exists, it is different from the one being moved, and removing a
    /// leaf **never erases another** — `remove` takes one out of the list, `collapse` replaces a
    /// branch with its only child and `absorb` lifts granddaughters into siblings; none of the
    /// three loses leaves.
    ///
    /// **It always returns a tree, and separately whether the movement happened.** The first
    /// version returned a `Result<Self, Self>`, and it was changed for two reasons that point to
    /// the same place: the `Err` carried the whole tree (clippy flags it), and above all **`Err`
    /// did not always mean the same thing** — in the three checks above the tree is untouched, and
    /// in the impossible path below it is not. A tuple does not invite reading «it failed» as
    /// «nothing changed».
    ///
    /// **The geometric floor is not checked here**, and it is not an oversight: the tree does not
    /// know the area. It is looked at by whoever has the rectangles in front of them, just as the
    /// splitting chord does with [`crate::tiling::can_split`].
    pub(crate) fn move_pane(mut self, moved: usize, target: usize, side: Side) -> (Self, bool) {
        if moved == target || !self.has_leaf(moved) || !self.has_leaf(target) {
            return (self, false);
        }

        // A leaf does not remove itself: if the root is the console itself, this tab has a single
        // one and there is no layout to rearrange.
        let Some(closing) = self.close(moved) else {
            return (self, false);
        };

        match self.split(target, side, closing.leaving) {
            None => (self, true),
            // **It cannot happen**, and even so it is not settled with an `expect`. The reason is
            // the same one for which closing does not panic either when it does not find the focus:
            // in here there is a **live** console, and the two comfortable ways out are the two
            // worst ones — dropping it kills its program, and a panic takes down the whole session,
            // all the other consoles included.
            //
            // So it is grafted onto the root, which leaves it in sight though in a place nobody
            // asked for, and it is said through the error output. **`Err` means «it did not move as
            // asked», not «the tree is untouched»**: along this path it did change, and that is why
            // it has to be said.
            Some(returned) => {
                eprintln!(
                    "the console could not be put where it was dropped after being taken out; it is left at \
                     the root so its program is not lost"
                );
                // **And this graft is not clean either, it is worth saying:** `make_a_branch` over
                // what here is a branch and not a leaf can leave two branches of the same direction
                // nested if the root tiles in the direction of `side`. It is accepted because this
                // path is unreachable and because an ugly shape is better than a dead program — but
                // it is not «the safe way out».
                self.make_a_branch(side, returned);
                (self, false)
            }
        }
    }

    /// Whether this node is the console with that name.
    fn is_the_pane_named(&self, name: usize) -> bool {
        matches!(self, Self::Leaf(pane) if pane.name == name)
    }

    /// This node's console, if it is a leaf and not a branch.
    fn pane(self) -> Option<Pane> {
        match self {
            Self::Leaf(pane) => Some(pane),
            Self::Branch { .. } => None,
        }
    }

    /// This node's children, if it is a branch and not a leaf.
    fn children(self) -> Option<Vec<Child>> {
        match self {
            Self::Branch { children, .. } => Some(children),
            Self::Leaf(_) => None,
        }
    }

    /// Removes the console named `name`, and leaves the tree with no branches that separate
    /// nothing.
    ///
    /// **It returns `None` when that console is not around here**, just as [`Node::split`] returns
    /// the one it has not placed: it is how «go on looking» is said without inventing anything.
    ///
    /// **A leaf does not remove itself**, and it is not a limitation left to be covered: the list
    /// it comes out of belongs to its branch. A tab with a single console has no branch, so closing
    /// that console is not rearranging a layout — it is closing the tab, and that is decided by the
    /// window.
    pub(crate) fn close(&mut self, name: usize) -> Option<Closing> {
        let closing = self.remove(name)?;
        // **The collapse goes here and not inside `remove`** because a branch cannot replace itself
        // while its list of children is borrowed. With the close already done, the borrow has
        // ended.
        self.collapse();
        Some(closing)
    }

    /// Takes out of this branch the leaf with that name, and says who the keyboard passes to.
    ///
    /// **It is inherited by the previous sibling if there was one, and if not by the next**; and
    /// inside it, the console left **stuck to the slot**: the last of the previous one, the first
    /// of the next. The rule is deterministic on purpose, and not «the first one to turn up while
    /// walking»: with siblings that are a single console both ways say the same thing, but with a
    /// sibling split inside, the difference is giving the keyboard to the console next door or to
    /// one at the other end of the tab.
    fn remove(&mut self, name: usize) -> Option<Closing> {
        let Self::Branch { children, .. } = self else {
            return None;
        };

        if let Some(at) = children
            .iter()
            .position(|child| child.node.is_the_pane_named(name))
        {
            let leaving = children
                .remove(at)
                .node
                .pane()
                .expect("the position has just been looked up among this branch's leaves");

            let neighbor = match at.checked_sub(1) {
                // The previous one stays in its place, and from it the **last** is inherited.
                Some(previous) => children
                    .get(previous)
                    .and_then(|child| child.node.leaves().last().copied()),
                // With no previous one the next is inherited, which has moved up to take the place
                // of the one that is leaving, and from it the **first**, for the same reason: it is
                // the one left stuck to the slot.
                None => children
                    .get(at)
                    .and_then(|child| child.node.leaves().first().copied()),
            };
            let focus = neighbor
                .expect(
                    "a branch is born with two children and collapses when it is left with one, so removing \
                         one leaves a sibling",
                )
                .name;

            return Some(Closing { leaving, focus });
        }

        // It was not among this branch's leaves: it goes on inside its children, and **it stops as
        // soon as one of them finds it**. It goes down through `close` and not through `remove` so
        // that the inner branch left with a single child also collapses.
        let mut found = None;
        for (which, child) in children.iter_mut().enumerate() {
            if let Some(closing) = child.node.close(name) {
                found = Some((which, closing));
                break;
            }
        }

        let (which, closing) = found?;
        // **The child that has just changed may have collapsed into a branch of this one's
        // direction**, and then its children have to be lifted to be siblings of the ones here. See
        // [`Node::absorb`].
        self.absorb(which);
        Some(closing)
    }

    /// A branch left with a single child stops separating anything: that child replaces it.
    ///
    /// **It is not cosmetic, even though it cannot be told apart on screen.** A branch with one
    /// child gives it its whole area and the tiling puts no divider on it, so it is painted the
    /// same. What changes is what happens **afterwards**: splitting in the same direction inserts
    /// as a sibling inside the branch that contains the leaf, so a leftover branch makes the new
    /// console be born inside it and not beside it — and then the outer divider again moves a block
    /// of two consoles, which is the `tmux` behavior this tree exists in order not to repeat.
    fn collapse(&mut self) {
        if let Self::Branch { children, .. } = self
            && children.len() == 1
        {
            // **The weight of the one that stays is discarded**, and it is the right thing: the
            // weight that tiles it among its aunts is the branch's, and that one is carried by the
            // entry its mother has for it. The inner one only said how it was tiled with siblings
            // that are no longer there.
            let only_child = children.pop().expect("a list of one has one");
            *self = only_child.node;
        }
    }

    /// If the child in that place tiles in the **same direction** as this branch, its children
    /// become siblings of the ones here, with the group's weight shared out among them.
    ///
    /// **Without this, closing leaves two branches of the same direction nested, and with them a
    /// divider that moves a block of two consoles** — exactly the `tmux` behavior the n-ary tree
    /// exists in order not to repeat. It is not a far-fetched case: three gestures are enough
    /// —split downwards, split beside, close the top one—, because the branch left with a single
    /// child is replaced by it and that child may be a branch of the outer one's direction.
    /// **Splitting cannot reach that shape** (inserting as a sibling is precisely what prevents
    /// it), so it is closing that enables it and closing that has to undo it.
    ///
    /// **The weights are scaled, not copied.** The group carried `w` of what this branch tiles and
    /// its children shared it out among themselves; giving each one `group_weight · (weight / Σ)`
    /// keeps **the tiling**: neither the block with respect to its aunts, nor the children among
    /// themselves. Copying them as they are would indeed move them, and in a direction that depends
    /// on how many siblings there were.
    ///
    /// **And «keeps the tiling» is not «nothing moves on screen», which is what this doc said
    /// before it was measured.** Lifting the children puts one more divider into this branch, and
    /// dividers are deducted from the area **before** tiling: the usable part goes down by one
    /// thickness for every extra child that is lifted, and that point is paid by all the ones here,
    /// including the ones that do not take part. Measured with
    /// `absorbing_preserves_the_geometry_of_the_consoles_that_do_not_move`: a console that took
    /// half of 1,200 points goes from 599 to 598. It is one point and it is unavoidable —the
    /// divider has to fit somewhere—, but **the strong claim was false**, and off it hangs the
    /// argument that checking the geometric floor against the tree as it is now is conservative: it
    /// is so because the floor is demanded of the branch that **is split**, not because nobody else
    /// moves.
    fn absorb(&mut self, which: usize) {
        let Self::Branch {
            orientation,
            children,
        } = self
        else {
            return;
        };

        let from_outside = *orientation;
        let from_inside = match children.get(which).map(|child| &child.node) {
            Some(Self::Branch { orientation, .. }) => *orientation,
            _ => return,
        };
        if from_inside != from_outside {
            return;
        }

        let group = children.remove(which);
        let group_weight = group.weight.value();
        let group_children = group
            .node
            .children()
            .expect("the orientation has just been read off a branch");
        let total: f32 = group_children
            .iter()
            .map(|child| child.weight.value())
            .sum();
        let lifted: Vec<Child> = group_children
            .into_iter()
            .map(|child| Child {
                // **It divides before multiplying, and it is not cosmetic.** `(group_weight · w) /
                // total` can overflow in the intermediate product even though the result is
                // representable; dividing first, the factor does not go above one and the result
                // does not go above `group_weight`. And the floor, because downwards it does reach
                // zero: see [`Weight::scaled`].
                weight: Weight::scaled(group_weight * (child.weight.value() / total)),
                node: child.node,
            })
            .collect();

        // **In the place the group took and in its order**, or what is painted would change place:
        // what was to the right of the block has to stay to the right of its two consoles.
        children.splice(which..which, lifted);
    }
}

/// What closing a pane leaves behind: the console that is leaving, and who the keyboard passes to.
///
/// **The console comes out of the tree without being dropped yet**, and that is deliberate: on
/// dropping it its program dies and its directory disappears, and before that what it had measured
/// of this frame has to be taken. If it left with its [`crate::measure::Timings`] inside, the subtraction
/// would hand its work to the UI framework — the figure D3 decides with. It is the same thing
/// closing a tab already does, and it cannot be done from inside the tree because the stopwatch
/// belongs to the window.
pub(crate) struct Closing {
    pub(crate) leaving: Pane,
    /// The name of the console that gets the keyboard. See [`Node::remove`].
    pub(crate) focus: usize,
}

/// A layout of consoles: what is seen inside a tab.
///
/// **It exists to separate what belongs to a layout from what belongs to the process.** The three
/// fields in here were fields of [`crate::Window`] when the window was a single layout; the ones that
/// stayed up there —the theme, the watcher, the modifiers, the measurement— do not describe how
/// some consoles are tiled, but how the program is.
///
/// **It is a transparent container on purpose, with no border of methods.** The seam that is needed
/// is given by the type itself: when the panes stop being two and become a tree, what changes is
/// the inside of this, and the list of tabs does not notice. Hiding it behind methods as well would
/// force putting [`crate::Window::handle_the_mouse`] and [`Tab::drag_the_divider`] inside the painting,
/// and those were taken out precisely so they could be checked without opening any window.
pub(crate) struct Tab {
    /// User-set display label, separate from the stable identity.
    pub(crate) label: Option<String>,
    /// How its consoles are tiled. See [`Node`].
    ///
    /// **It replaces the array of two and the loose fraction that used to be here.** That array
    /// said in the type that the consoles were exactly two, which was true and was useful while it
    /// lasted; the tree says they can be however many and **keeps the guarantee that really
    /// mattered**: that what is painted covers the area with no gaps.
    pub(crate) root: Node,
    /// Which one the keyboard goes to, **by its name and not by its place**.
    ///
    /// The index served while the consoles were two and fixed. As soon as they can be closed, the
    /// ones beside them move up a place and the focus would jump on its own to another console —
    /// the same problem the report already solved by calling every pane by a name that is not
    /// reused.
    ///
    /// **It is not egui's focus.** The framework's is a keyboard focus per widget, for tabbing
    /// between controls; this one is geographic and the application decides it. Confusing them
    /// leaves the keyboard in the hands of whoever does not route it.
    pub(crate) focus: usize,
    /// What it is called in the report, and **it does not change even if others are closed**.
    ///
    /// **It is the list index that is no good for this.** On closing a tab, the ones to its right
    /// move up a place: the one that was number 2 becomes number 1, and the report published on
    /// leaving would end up putting another tab's name on a table of numbers — with the spans of
    /// the one that produced them. Whoever reads that report has no way of noticing. It is assigned
    /// when the tab is opened and never reused.
    pub(crate) name: usize,
}

impl Tab {
    /// The consoles the layout says, with the user's shell inside each one.
    ///
    /// **The first pane name comes from outside**, just like the tab's: the counter that guarantees
    /// they are not repeated belongs to the window, because the panes of two different tabs also
    /// have to be told apart in the report.
    ///
    /// **It receives the first one and not the whole list, and that was decided by a review.** With
    /// a list and a layout there were **two sources for the same datum** —how many consoles there
    /// are—, and with two the wrong one can be taken: whoever called with three names and a layout
    /// of four would open a tab that is neither of the two. Counted here, there is nothing to keep
    /// aligned.
    pub(crate) fn new_counting_reads(
        ctx: &egui::Context,
        name: usize,
        first_pane_name: usize,
        layout: &OpeningLayout,
        reads: crate::control::ReadActivity,
    ) -> term_core::Result<Self> {
        // **If any console fails the tab is not opened**, and the `?` over the `collect` is what
        // achieves it: the same criterion the age-old two already had.
        let how_many = layout.how_many_panes();
        let panes: Vec<Pane> = (first_pane_name..first_pane_name + how_many)
            .map(|name| Pane::new_counting_reads(ctx, name, reads.clone()))
            .collect::<term_core::Result<_>>()?;

        Ok(Self::with_panes(panes, layout, name))
    }

    /// How a tab is opened: the focus on the left and the divider where [`crate::tiling::INITIAL_FRACTION`] says.
    ///
    /// It exists apart from [`Tab::new_counting_reads`] so that the tests come in through here with consoles of
    /// their own instead of building the struct by hand — otherwise, the day opening a tab means
    /// something more, the tests would go on opening the old one.
    pub(crate) fn with_panes(panes: Vec<Pane>, layout: &OpeningLayout, name: usize) -> Self {
        // **The focus starts on the first one** because typing before the first click always
        // happens when the window is opened, and with a focus pointing at nobody those keys would
        // be lost. With the age-old layout, the first one is the one on the left.
        //
        // **`expect` and not a silent fallback**, for the same reason as in [`weights_of`]: a tab
        // with no console at all does not exist —[`opening_layout_of`] rejects zero on both axes
        // and [`Tab::new_counting_reads`] opens one console per name—, and a fallback would turn that
        // impossibility into something plausible. Zero would be the worst of all: it is the name of
        // **the first console of the first tab**, so the focus would point at a console of another
        // tab and the keys would go where nobody expects them. Besides, with no leaf at all,
        // [`in_layout`] would return a branch with no children, with a weight sum of zero and a
        // `NaN` in the painting.
        let focus = panes
            .first()
            .expect("a tab needs at least one console")
            .name;

        Self {
            root: in_layout(panes, layout.rows),
            label: None,
            focus,
            name,
        }
    }

    /// Its consoles, from the first to the last in the order in which they are painted.
    pub(crate) fn panes(&self) -> Vec<&Pane> {
        self.root.leaves()
    }

    pub(crate) fn panes_mut(&mut self) -> Vec<&mut Pane> {
        self.root.leaves_mut()
    }

    /// The console called that, if it is still in this tab.
    pub(crate) fn pane_called(&mut self, name: usize) -> Option<&mut Pane> {
        self.panes_mut().into_iter().find(|pane| pane.name == name)
    }

    /// What this tab's consoles claim most, if any of them claims anything.
    ///
    /// **It is the maximum, and the order is set by the type itself**: `wait` beats `block` and
    /// `block` beats `done`, as the contract says, and at equal class the one somebody is holding
    /// up wins. Deciding it here with an `if` would be a second copy of that rule.
    ///
    /// **The ones that do not claim are flattened before comparing.** Looking for the maximum over
    /// the `Option`s as they are returns «something that contains nothing» when none of them claims
    /// —a `Some(None)`— and whoever read it asking whether there is anything would mark the card
    /// of a tab where nobody claims.
    pub(crate) fn attention(&self, now: SystemTime) -> Option<panel::AttentionState> {
        self.panes()
            .iter()
            .filter_map(|pane| pane.effective_attention(now))
            .max()
    }

    /// Where the console that has the keyboard falls inside the painted list, if it is still alive.
    ///
    /// **It returns a position and not the console** because whoever asks needs to match it with
    /// the geometry, which travels in a separate list and in the same order.
    pub(crate) fn where_the_focus_is(&self) -> Option<usize> {
        self.panes().iter().position(|pane| pane.name == self.focus)
    }

    /// What part of its span the first of the two that **each divider** separates takes, in the
    /// same order in which they are placed and in which they are dragged.
    ///
    /// **A list and not «the fraction», since splitting is possible.** While the layout was two
    /// columns, publishing the first one and calling it «the divider» told the truth; with three
    /// consoles there are two dividers, and a line of the report that only talks about one of them
    /// is exactly the wrongly labelled number this project has already paid for once.
    pub(crate) fn divider_fractions(&self) -> Vec<f32> {
        let mut fractions = Vec::new();
        Self::fractions_in(&self.root, &mut fractions);
        fractions
    }

    /// A node's fractions, **in the order in which [`Node::place`] delivers its dividers**: first
    /// this branch's cuts, then its children's. It is the same numbering [`Tab::move_divider`]
    /// comes in by, and they have to match or the report would say about one divider what happened
    /// to another.
    fn fractions_in(node: &Node, fractions: &mut Vec<f32>) {
        let Node::Branch { children, .. } = node else {
            return;
        };

        for cut_along in 0..children.len().saturating_sub(1) {
            let between_the_two =
                children[cut_along].weight.value() + children[cut_along + 1].weight.value();
            fractions.push(children[cut_along].weight.value() / between_the_two);
        }
        for child in children {
            Self::fractions_in(&child.node, fractions);
        }
    }

    /// Moves the divider with the pointer, and returns whether the fraction changed in this pass.
    ///
    /// It lives apart from the painting for the same reason as [`crate::Window::handle_the_mouse`]:
    /// inside the interface pass it cannot be checked without opening a window.
    ///
    /// **It is the tab's and not the window's, and that is where it came from.** It lives beside
    /// [`Tab::move_divider`] — the one thing it ends up calling — because a divider shares out the
    /// weights of one tab and nothing above it ever finds out. The move is the one the delivery
    /// before this made with the panel's border, which went to the module that owns panels: a
    /// gesture over a piece of chrome belongs with that piece, and `pointer.rs` keeps the ones
    /// whose subject is deciding **whose** a gesture is. It also pays for that file's own lines
    /// under **D28**, which is the order this project takes pieces out in.
    ///
    /// **The fraction comes out of the pointer's absolute position, not of its displacement.** With
    /// the displacement, every little bit the floor's clip discards would be lost for ever: the
    /// pointer would keep advancing and the fraction would fall behind, so on coming back the
    /// divider would not be where the mouse is. With the absolute position, the divider chases the
    /// pointer and the clip only decides where it plants itself.
    ///
    /// And there is a second reason, a defence: **the interface pass can run twice in one frame**.
    /// egui 0.36 gives the repeated pass a zero drag delta, but an egui that replayed the frame's
    /// events would have the displacement applied again, moving the divider twice as far. With the
    /// absolute position a second pass computes the same thing and changes nothing, whichever egui
    /// runs it — the same defence the keyboard keeps by consuming its events.
    pub(crate) fn drag_the_divider(
        &mut self,
        ui: &egui::Ui,
        which: usize,
        divider: Divider,
    ) -> bool {
        // `Sense::DRAG` and not `Sense::drag()`: the second adds `FOCUSABLE`, that is, the divider
        // would ask for egui's keyboard focus and would compete with the application's geographic
        // routing. It is the same trap that already cost a case with the click.
        //
        // **The identifier carries which one it is, and before it was a constant.** With a single
        // divider it made no difference; with several, a shared identifier turns them all into
        // **the same control** as far as egui is concerned, so dragging one would move another or
        // none at all. It is exactly what `handle_the_mouse` already solved by indexing its own,
        // one line away.
        //
        // **It does not carry which tab, and that is a constraint on the callers rather than an
        // oversight.** While this was a method of the window it could only ever reach the active
        // tab; as a method of the tab it could be called on any of them, and two tabs dragging
        // dividers of the same number would be one control to egui. Whoever gives a background tab
        // a divider to drag has to put the tab in here.
        let id = ui.id().with(("divider", which));
        let response = ui
            .interact(divider.rect, id, egui::Sense::DRAG)
            // The cursor is kept **while it is being dragged too**. It is not because the pointer
            // leaves the line —egui takes the widget being dragged as «hovered», wherever the
            // pointer is—: it is because of the frame in which it is released, where the drag has
            // already finished and the pointer may have ended up outside. With `on_hover_cursor`
            // there the cursor flickers to an arrow just as the maneuver ends.
            .on_hover_and_drag_cursor(divider.orientation.cursor());

        let Some(pointer) = response.interact_pointer_pos() else {
            return false;
        };

        // **Measured inside the span of the two it separates**, not of the whole area: moving a
        // divider shares out the weight of its two neighbors keeping their sum, and the rest never
        // find out.
        let slice = divider.between_the_two;
        let usable = divider.orientation.length_of(slice) - DIVIDER_THICKNESS;
        // With nothing to share out there is no fraction to choose, and with a **negative**
        // `usable` the division changes sign: the divider would go to the side opposite the mouse.
        // The tiling is left as the last real drag left it, which is what the user asked for the
        // last time it made sense.
        if usable <= 0.0 {
            return false;
        }

        // The clip to `0..1` **is not for the geometry** —[`tile`] takes care of that on every
        // frame, with its own floor— but for the state: without it, dragging past the edge would
        // store a fraction of 5, which the exit report would publish as «at 500 %» and which would
        // force travelling the way back before the divider moved.
        let offset = divider.orientation.offset_from_the_start(slice, pointer);
        let new = (offset / usable).clamp(0.0, 1.0);
        // The guard above lets through a `usable` that is **not a number**, because `NaN <= 0.0` is
        // false, and out of that comes a NaN fraction that the clip does not catch either: a
        // `clamp` over a NaN returns NaN. And a NaN stored here never leaves — the window would
        // stop painting and neither widening it nor dragging again would recover it. The area that
        // is not a number is not hypothetical: it is the case [`CellMetrics::cells_in`] already
        // documents, while the desktop has not yet said how big the window is.
        if !new.is_finite() {
            return false;
        }

        self.move_divider(which, new)
    }

    /// Moves divider number `which` to that fraction. Says whether anything changed.
    ///
    /// **It shares out only between the two the divider separates and keeps what they add up to**,
    /// which is the tree's rule: moving a divider does not touch the siblings that are beyond. With
    /// two children it makes no difference; with three it is what separates this behavior from that
    /// of a terminal where dragging a divider moves everything to its right.
    pub(crate) fn move_divider(&mut self, which: usize, fraction: f32) -> bool {
        let mut seen = 0;
        Self::move_in(&mut self.root, which, fraction, &mut seen).unwrap_or(false)
    }

    /// Looks for divider number `which` walking the tree **in the same order in which they are
    /// placed**, and moves it. `seen` is how many have been left behind.
    ///
    /// **They are numbered by walking instead of keeping a path down to the branch**, and it is the
    /// same invariant the painting already rests on: the leaves and their rectangles are matched by
    /// position because the two walks go in the same order. An explicit path would be a second
    /// mechanism saying the same thing, with the possibility of the two disagreeing.
    ///
    /// **It returns `None` when the divider was not in this node, and `Some(whether anything
    /// changed)` when it was**, and those are two different questions that used to travel in a
    /// single boolean. Finding it and not moving it —the frame in which the pointer has not moved,
    /// or any in which the clip leaves it where it was— read as «it is not here», so the search
    /// went on through the next subtree: as the count had not advanced, the sibling's divider
    /// matched the same number and **moved on its own**. While the layout was a branch of two
    /// leaves that path was never walked; with two sibling branches —two presses of split— it was.
    fn move_in(node: &mut Node, which: usize, fraction: f32, seen: &mut usize) -> Option<bool> {
        let Node::Branch { children, .. } = node else {
            return None;
        };

        for cut_along in 0..children.len().saturating_sub(1) {
            if *seen == which {
                let between_the_two =
                    children[cut_along].weight.value() + children[cut_along + 1].weight.value();
                let [first, second] =
                    weights_of(fraction).map(|weight| weight.value() * between_the_two);
                // **It is compared against what is going to be saved, not against what has just
                // been calculated.** With a weight already on the floor, the calculation gives a
                // smaller number and the floor returns it to the same place: comparing against the
                // calculation, every frame of the drag would say «it changed» and would redo the
                // tiling to leave everything as it was.
                let changed = children[cut_along].weight != Weight::scaled(first);

                // **The fraction is clipped; the product is not covered by that.** `weights_of`
                // guarantees that neither of the two fractions is zero, but what is saved is the
                // fraction **times what the two neighbors add up to**, and that sum shrinks with
                // every drag: it is precisely the path that leads to the tiny weights. The floor is
                // set by [`Weight::scaled`].
                children[cut_along].weight = Weight::scaled(first);
                children[cut_along + 1].weight = Weight::scaled(second);

                return Some(changed);
            }
            *seen += 1;
        }

        // It was not in this branch: it goes on inside its children, in the order in which they are
        // placed, and **it stops as soon as one of them finds it** — whether it has moved it or
        // not.
        for child in children {
            if let Some(changed) = Self::move_in(&mut child.node, which, fraction, seen) {
                return Some(changed);
            }
        }

        None
    }
}

impl From<Orientation> for session::StoredOrientation {
    fn from(orientation: Orientation) -> Self {
        match orientation {
            Orientation::Columns => Self::Columns,
            Orientation::Rows => Self::Rows,
        }
    }
}

impl From<session::StoredOrientation> for Orientation {
    fn from(orientation: session::StoredOrientation) -> Self {
        match orientation {
            session::StoredOrientation::Columns => Self::Columns,
            session::StoredOrientation::Rows => Self::Rows,
        }
    }
}

/// Converts a live node to inert session metadata.
fn describe_node(node: &Node) -> session::StoredNode {
    match node {
        Node::Leaf(pane) => session::StoredNode::Leaf(session::StoredLeaf {
            cwd: pane.restoration.directory.clone(),
            panel_width: pane.panel_width,
            folded: pane.folded,
            label: pane.label.clone(),
        }),
        Node::Branch {
            orientation,
            children,
        } => session::StoredNode::Branch {
            orientation: (*orientation).into(),
            children: children
                .iter()
                .map(|child| session::StoredChild {
                    weight: child.weight.value(),
                    node: child.node.describe(),
                })
                .collect(),
        },
    }
}

impl Node {
    /// How this node is described so that it can be built again.
    pub(crate) fn describe(&self) -> session::StoredNode {
        describe_node(self)
    }

    /// The tree that description describes, **opening one console per leaf**.
    ///
    /// **It is the only place in the program where a description turns into processes**, and that
    /// is why it receives a [`session::Validated`] through the root and not just any node: the
    /// shape is already checked before getting here, so this path cannot build a branch with one
    /// child or an impossible weight.
    ///
    /// **If one leaf cannot be opened, none is installed.** The error goes up and the caller is not
    /// left with half a tab — the same atomicity opening the window already has, and the same that
    /// **D15** demands for moving a pane.
    pub(crate) fn materialize(
        ctx: &egui::Context,
        saving: &session::StoredNode,
        program: &str,
        next_name: &mut usize,
        reads: &crate::control::ReadActivity,
    ) -> term_core::Result<Self> {
        match saving {
            session::StoredNode::Leaf(leaf) => {
                let mut pane =
                    crate::restoration::open_leaf(ctx, leaf, program, *next_name, reads.clone())?;
                *next_name += 1;
                // **The width is raised to the drag floor, and a review asked for this.** A file
                // can bring a zero, and **no gesture can leave one**: asking for less than the
                // floor means folding, so the drag never saves a width below the floor. A zero with
                // `folded` false leaves a panel of zero points that **cannot be grabbed again**
                // —grabbing requires width— and, since the only one that writes this field is the
                // drag itself, it stays that way for ever and in every session. By clipping here,
                // what comes back from the disk is always a state a gesture could have left.
                pane.panel_width = leaf.panel_width.max(panel::DRAG_FLOOR);
                pane.folded = leaf.folded;
                pane.label = leaf.label.clone();
                Ok(Self::Leaf(pane))
            }
            session::StoredNode::Branch {
                orientation,
                children,
            } => {
                let mut parts = Vec::with_capacity(children.len());
                for child in children {
                    // The weight goes back through its constructor. It was already validated, so it
                    // cannot fail here; `expect` instead of a fallback because a silent fallback
                    // would turn an impossibility into a tiling nobody asked for.
                    // **An error and not an `expect`, and a review asked for that.** The comment
                    // said that the weight arrives validated because the root receives a valid
                    // document, and **it is true only for the root**: these two recursive functions
                    // receive bare nodes, so the day another path of the crate calls here with
                    // something that did not go through the validation, a file with `"weight": 0`
                    // **took down the application**. It is the invariant that depends on the memory
                    // of whoever writes, which is exactly what this module rejects by doctrine.
                    let weight = Weight::new(child.weight)
                        .ok_or("the layout carries something that cannot be a weight")?;
                    parts.push((
                        weight,
                        Self::materialize(ctx, &child.node, program, next_name, reads)?,
                    ));
                }
                Ok(in_branch_with_weights((*orientation).into(), parts))
            }
        }
    }
}

impl Tab {
    /// How this tab is described.
    ///
    /// **Neither the focus nor the name**: the window opens on the first tab, and the names are
    /// handed out by a window counter that reuses none — restoring an old one would make the report
    /// put another console's name on a table of numbers.
    pub(crate) fn describe(&self) -> session::StoredTab {
        session::StoredTab {
            root: self.root.describe(),
            label: self.label.clone(),
        }
    }

    /// The tab that description describes, with its consoles open.
    pub(crate) fn materialize(
        ctx: &egui::Context,
        saved: &session::StoredTab,
        program: &str,
        next_name: &mut usize,
        name: usize,
        reads: &crate::control::ReadActivity,
    ) -> term_core::Result<Self> {
        let root = Node::materialize(ctx, &saved.root, program, next_name, reads)?;
        // **The focus on the first one, just like when opening a new tab**, and for the same
        // reason: typing before the first click always happens, and with a focus that points at
        // nobody those keys would be lost. That there is at least one leaf is guaranteed by the
        // document's validation.
        let focus = root
            .leaves()
            .first()
            .ok_or("a restored tab with no console in it")?
            .name;

        Ok(Self {
            root,
            focus,
            name,
            label: saved.label.clone(),
        })
    }
}
