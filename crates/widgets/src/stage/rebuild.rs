use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use indextree::NodeId;

use crate::{
    forest::{Forest, Get, GetCarefully},
    types::{dirty_flags::DirtyFlags, identifiers::WidgetKey},
    widget::WidgetGetType,
};

pub trait WidgetDiff {
    fn diff(&self, other: &dyn std::any::Any) -> DirtyFlags;
}

#[derive(Debug, Default)]
pub(crate) struct RebuildContext {
    matched_keyed_nodes: MatchedKeyedNodes,
    operation_set: Vec<RebuildOperation>,
}

impl RebuildContext {
    pub(crate) fn operation_set(&self) -> impl Iterator<Item = &RebuildOperation> {
        self.operation_set.iter()
    }
}

/// It assumes that both of widget trees (main and pending) have unique [WidgetKey]s. In this case,
/// there must only both unique [NodeId] for each [WidgetKey].
#[derive(Default, Debug)]
struct MatchedKeyedNodes {
    keyed_nodes: HashMap<WidgetKey, MatchedNodes>,
    matched_nodes: HashSet<NodeId>,
}

#[derive(Debug, Clone, Copy)]
struct MatchedNodes {
    node_in_main_tree: NodeId,
    node_in_pending_tree: NodeId,
}

impl MatchedKeyedNodes {
    fn insert(&mut self, key: WidgetKey, matched_nodes: MatchedNodes) {
        assert!(
            self.keyed_nodes.insert(key, matched_nodes).is_none(),
            "Duplicate WidgetKey found in a main tree."
        );
        self.matched_nodes.insert(matched_nodes.node_in_main_tree);
        self.matched_nodes
            .insert(matched_nodes.node_in_pending_tree);
    }

    fn contains_node_id(&self, node_id: &NodeId) -> bool {
        self.matched_nodes.contains(node_id)
    }
}

#[derive(Debug)]
pub(crate) enum RebuildOperation {
    Initialize {
        node_id: NodeId,
    },
    Deinitialize {
        node_id: NodeId,
    },
    Reuse {
        old_node_id: NodeId,
        new_node_id: NodeId,
    },
}

pub trait RebuildTree {
    fn rebuild_tree(&mut self);
}

pub(crate) fn scan_forest<Id, Node>(forest: &Forest<Id, Node>, context: &mut RebuildContext)
where
    Id: Hash + Eq,
    Node: Get<Id> + GetCarefully<WidgetKey> + WidgetGetType,
{
    let mut keyed_nodes = HashMap::new();

    if let Some(main_tree_root) = forest.main_tree_root() {
        for node_edge in forest.traverse(main_tree_root) {
            if let indextree::NodeEdge::Start(node_id) = node_edge {
                let node = forest
                    .node(&node_id)
                    .expect("The node must exist and belong to a main tree!");

                if let Some(widget_key) = node.get_carefully() {
                    assert!(
                        keyed_nodes.insert(widget_key.clone(), node_id).is_none(),
                        "Duplicate WidgetKey found in a main tree."
                    );
                }
            }
        }
    }

    if let Some(pending_tree_root) = forest.pending_tree_root() {
        for node_edge in forest.traverse(pending_tree_root) {
            if let indextree::NodeEdge::Start(node_id) = node_edge {
                let node = forest
                    .node(&node_id)
                    .expect("The node must exist and belong to a pending tree!");

                let Some(widget_key) = node.get_carefully() else {
                    continue;
                };

                if let Some(node_id_in_main_tree) = keyed_nodes.get(widget_key) {
                    let node_in_main_tree = forest
                        .node(node_id_in_main_tree)
                        .expect("The node must exist and belong to a main tree!");

                    if node_in_main_tree.get_type() == node.get_type() {
                        context.matched_keyed_nodes.insert(
                            widget_key.clone(),
                            MatchedNodes {
                                node_in_main_tree: *node_id_in_main_tree,
                                node_in_pending_tree: node_id,
                            },
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn positional_diffing<Id, Node>(
    forest: &Forest<Id, Node>,
    main_node_id: Option<&NodeId>,
    pending_node_id: Option<&NodeId>,
    context: &mut RebuildContext,
) where
    Id: Hash + Eq,
    Node: Get<Id> + WidgetGetType,
{
    match (main_node_id, pending_node_id) {
        (None, None) => (),
        (Some(main_node), None) => {
            context.operation_set.push(RebuildOperation::Deinitialize {
                node_id: *main_node,
            });

            for node in forest.children_of(main_node) {
                if context.matched_keyed_nodes.contains_node_id(&node) {
                    continue;
                }

                positional_diffing(forest, Some(&node), None, context);
            }
        }
        (None, Some(pending_node)) => {
            context.operation_set.push(RebuildOperation::Initialize {
                node_id: *pending_node,
            });

            for node in forest.children_of(pending_node) {
                if context.matched_keyed_nodes.contains_node_id(&node) {
                    continue;
                }

                positional_diffing(forest, None, Some(&node), context);
            }
        }
        (Some(main_node_id), Some(pending_node_id)) => {
            let main_node = forest
                .node(main_node_id)
                .expect("There must be a node in a main tree!");
            let pending_node = forest
                .node(pending_node_id)
                .expect("There must be a node in a pending tree!");

            if main_node.get_type() == pending_node.get_type() {
                context.operation_set.push(RebuildOperation::Reuse {
                    old_node_id: *main_node_id,
                    new_node_id: *pending_node_id,
                })
            } else {
                context.operation_set.push(RebuildOperation::Deinitialize {
                    node_id: *main_node_id,
                });
                context.operation_set.push(RebuildOperation::Initialize {
                    node_id: *pending_node_id,
                });
            }

            fn next_unmatched_by_key_node(
                iterator: &mut impl Iterator<Item = NodeId>,
                context: &RebuildContext,
            ) -> Option<NodeId> {
                for node_id in iterator.by_ref() {
                    if context.matched_keyed_nodes.contains_node_id(&node_id) {
                        continue;
                    }

                    return Some(node_id);
                }

                None
            }

            let mut main_node_children = forest.children_of(main_node_id);
            let mut pending_node_children = forest.children_of(pending_node_id);

            let mut main_node_child = next_unmatched_by_key_node(&mut main_node_children, context);
            let mut pending_node_child =
                next_unmatched_by_key_node(&mut pending_node_children, context);

            while main_node_child.is_some() || pending_node_child.is_some() {
                positional_diffing(
                    forest,
                    main_node_child.as_ref(),
                    pending_node_child.as_ref(),
                    context,
                );

                main_node_child = next_unmatched_by_key_node(&mut main_node_children, context);
                pending_node_child =
                    next_unmatched_by_key_node(&mut pending_node_children, context);
            }
        }
    }
}

pub(crate) fn keyed_diffing<Id, Node>(forest: &Forest<Id, Node>, context: &mut RebuildContext)
where
    Id: Hash + Eq,
    Node: Get<Id> + WidgetGetType,
{
    let matched_nodes = context
        .matched_keyed_nodes
        .keyed_nodes
        .values()
        .copied()
        .collect::<Vec<_>>();

    for MatchedNodes {
        node_in_main_tree,
        node_in_pending_tree,
    } in matched_nodes
    {
        positional_diffing(
            forest,
            Some(&node_in_main_tree),
            Some(&node_in_pending_tree),
            context,
        );
    }
}
