use std::collections::HashMap;

use indextree::Arena;
use shared::unique::Unique;

pub use indextree::NodeId;

/// A contract requires to retrieve some `T` from a type.
pub trait Get<T> {
    /// Retrieves a `T` value from a type.
    fn get(&self) -> T;
}

/// A contract requires to retrieve some `T` from a type if exists, otherwise none.
pub trait GetCarefully<T> {
    /// Retrieves a `Option<&T>` value from a type.
    fn get_carefully(&self) -> Option<&T>;
}

/// A forest of geeneric trees of nodes stored in an arena.
///
/// It allows fast use, retrieve, change and delete nodes within the same forest, managing complex
/// things internally and provides convenient API externally.
///
/// The important note that you must know is that a forest have a single main root, and each
/// operation where root doesn't mention implies main root. Otherwise explicitly mention an other
/// root.
pub(crate) struct Forest<Id, Node>
where
    Id: std::hash::Hash + Eq,
    Node: Get<Id>,
{
    arena: Arena<Node>,
    main_root: Option<NodeId>,
    pending_root: Option<NodeId>,
    relations: Relations<Id, NodeId>,
}

impl<Id, Node> Default for Forest<Id, Node>
where
    Id: std::hash::Hash + Eq,
    Node: Get<Id>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Id, Node> Forest<Id, Node>
where
    Id: std::hash::Hash + Eq,
    Node: Get<Id>,
{
    pub(crate) fn new() -> Self {
        Self {
            arena: Arena::new(),
            main_root: None,
            pending_root: None,
            relations: Relations::default(),
        }
    }

    pub(crate) fn set_pending_tree_root(&mut self, pending_tree_root: NodeId) {
        self.pending_root = Some(pending_tree_root);
    }

    pub(crate) fn create_node(&mut self, node: Node) -> NodeId {
        self.arena.new_node(node)
    }

    pub(crate) fn append_node(&mut self, parent: NodeId, child: NodeId) {
        // TODO: get cardinality of current node to check whether is possible to add child or not
        parent.append(child, &mut self.arena);
    }

    pub(crate) fn make_relation(&mut self, id: Id, node_id: NodeId)
    where
        Id: Clone,
    {
        self.relations.make_relation(id, node_id);
    }

    pub(crate) fn remove_relation(&mut self, id: Id) {
        self.relations.remove_relation_by_left(&id);
    }

    pub(crate) fn main_tree_root(&self) -> Option<&NodeId> {
        self.main_root.as_ref()
    }

    pub(crate) fn main_tree_root_id(&self) -> Option<&Id> {
        self.main_root
            .as_ref()
            .and_then(|node_id| self.relations.get_by_right(node_id))
    }

    pub(crate) fn pending_tree_root(&self) -> Option<&NodeId> {
        self.pending_root.as_ref()
    }

    /// Promotes a pending tree root to main tree root.
    ///
    /// NOTE: this operation entirely destroys a main tree immediately, if pending tree root exists.
    /// So, be sure that all required operations are performed (about initialization,
    /// deinitialization and transferring responsibility to new nodes) before calling this method.
    pub(crate) fn promote_pending_tree_root(&mut self) {
        if let Some(root) = self.pending_root {
            if let Some(main_root) = self.main_root {
                main_root.remove_subtree(&mut self.arena);
            }

            self.main_root = Some(root);
            self.pending_root = None;
        }
    }

    pub(crate) fn node(&self, node_id: &NodeId) -> Option<&Node> {
        self.arena.get_data(*node_id)
    }

    pub(crate) fn node_mut(&mut self, node_id: &NodeId) -> Option<&mut Node> {
        self.arena.get_data_mut(*node_id)
    }

    pub(crate) fn node_by_id(&self, id: &Id) -> Option<&Node> {
        self.relations
            .get_by_left(id)
            .and_then(|node_id| self.node(node_id))
    }

    pub(crate) fn node_by_id_mut(&mut self, id: &Id) -> Option<Unique<Node>> {
        self.relations.get_by_left(id).and_then(|node_id| {
            self.arena
                .get_data_mut(*node_id)
                .map(|val| unsafe { Unique::from_mut(val) })
        })
    }

    pub(crate) fn parent_id_by_id(&self, id: Id) -> Option<Id>
    where
        Id: Clone,
    {
        self.relations
            .get_by_left(&id)
            .and_then(|node_id| node_id.parent(&self.arena))
            .and_then(|parent_node_id| self.relations.get_by_right(&parent_node_id))
            .cloned()
    }

    pub(crate) fn parent_by_id(&self, id: Id) -> Option<&Node> {
        self.relations
            .get_by_left(&id)
            .and_then(|node_id| node_id.parent(&self.arena))
            .and_then(|parent_node_id| self.node(&parent_node_id))
    }

    pub(crate) fn parent_by_id_mut(&mut self, id: Id) -> Option<Unique<Node>> {
        self.relations
            .get_by_left(&id)
            .and_then(|node_id| node_id.parent(&self.arena))
            .and_then(|parent_node_id| {
                self.arena
                    .get_data_mut(parent_node_id)
                    .map(|val| unsafe { Unique::from_mut(val) })
            })
    }

    pub(crate) fn children_of(&self, node_id: &NodeId) -> indextree::Children<'_, Node> {
        node_id.children(&self.arena)
    }

    pub(crate) fn childrens_indices_by_id(&self, id: Id) -> Vec<Id>
    where
        Id: Clone,
    {
        self.relations
            .get_by_left(&id)
            .map(|node_id| {
                self.children_of(node_id)
                    .flat_map(|child_node_id| self.relations.get_by_right(&child_node_id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn children_by_id(&self, id: Id) -> Vec<&Node> {
        self.relations
            .get_by_left(&id)
            .map(|node_id| {
                self.children_of(node_id)
                    .flat_map(|child_node_id| self.node(&child_node_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn children_by_id_mut(&mut self, id: Id) -> Vec<Unique<Node>> {
        self.relations
            .get_by_left(&id)
            .copied()
            .map(|node_id| {
                // INFO: here children twice collects into Vec<NodeId> and Vec<Unique<Node>> because
                // borrow checker doesn't allow immutable and mutable access to &mut self at the
                // same time.
                self.children_of(&node_id)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .flat_map(|child_node_id| {
                        self.arena
                            .get_data_mut(child_node_id)
                            .map(|val| unsafe { Unique::from_mut(val) })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn traverse(&self, node_id: &NodeId) -> indextree::Traverse<'_, Node> {
        node_id.traverse(&self.arena)
    }

    pub(crate) fn remove_standalone_subtrees(&mut self) {
        self.arena
            .roots()
            .filter(|&root_node_id| {
                self.main_root != Some(root_node_id) && self.pending_root != Some(root_node_id)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|standalone_root_id| standalone_root_id.remove_subtree(&mut self.arena));
    }
}

struct Relations<Left, Right>
where
    Left: std::hash::Hash + Eq,
    Right: std::hash::Hash + Eq,
{
    left_to_right: HashMap<Left, Right>,
    right_to_left: HashMap<Right, Left>,
}

impl<Left, Right> Default for Relations<Left, Right>
where
    Left: std::hash::Hash + Eq,
    Right: std::hash::Hash + Eq,
{
    fn default() -> Self {
        Self {
            left_to_right: HashMap::new(),
            right_to_left: HashMap::new(),
        }
    }
}

impl<Left, Right> Relations<Left, Right>
where
    Left: std::hash::Hash + Eq,
    Right: std::hash::Hash + Eq,
{
    fn make_relation(&mut self, left: Left, right: Right)
    where
        Left: Clone,
        Right: Clone,
    {
        self.left_to_right.insert(left.clone(), right.clone());
        self.right_to_left.insert(right, left);
    }

    fn remove_relation_by_left(&mut self, left: &Left) {
        if let Some(right) = self.left_to_right.remove(left) {
            self.right_to_left.remove(&right);
        }
    }

    #[allow(unused)]
    fn remove_relation_by_right(&mut self, right: &Right) {
        if let Some(left) = self.right_to_left.remove(right) {
            self.left_to_right.remove(&left);
        }
    }

    fn get_by_left(&self, left: &Left) -> Option<&Right> {
        self.left_to_right.get(left)
    }

    fn get_by_right(&self, right: &Right) -> Option<&Left> {
        self.right_to_left.get(right)
    }
}
