/// Implements subgraph isomorphism algorithms two ways:
/// as described in https://www.cs.bgu.ac.il/~dekelts/publications/subtree.pdf
/// Another thing to consider, but is not implemented here, is
/// http://chasewoerner.org/popl87.pdf
///
use super::graph_utils::{find_leaves, find_root, has_property_subset};
use indexmap::map::IndexMap;
use pathfinding::directed::edmonds_karp::*;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::DfsPostOrder;
use petgraph::{Incoming, Outgoing};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
extern crate test;

pub type GraphType = Graph<(String, IndexMap<u64, String>), ()>;
pub type SetSType = IndexMap<SetSKey, IndexMap<NodeIndex, Option<Vec<(NodeIndex, NodeIndex)>>>>;
// -------------- Shamir Isomorphism Algorithm Helper Functions---------------
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct SetSKey {
    pub val1: NodeIndex,
    pub val2: NodeIndex,
}
impl Serialize for SetSKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?},{:?}", self.val1.index(), self.val2.index()))
    }
}

impl<'de> Deserialize<'de> for SetSKey {
    fn deserialize<D>(deserializer: D) -> Result<SetSKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let mut iterator = s.split(',');
        let first_val = iterator.next().unwrap().parse::<usize>().unwrap();
        let second_val = iterator.next().unwrap().parse::<usize>().unwrap();

        Ok(SetSKey {
            val1: NodeIndex::new(first_val),
            val2: NodeIndex::new(second_val),
        })
    }
}


/// Given two sets of nodes, set x from graph g, and set y from graph h,
/// creates a flow graph with the source connected to all nodes in x and
/// the sink connected to all nodes in y.  Edges between x and y are computed
/// based on if their set (in set_s) contains u_null.  Then we compute
/// the flow of that graph, which is equivalent to the maximum matching.
/// We also have the target size of the maximum matching - the size we are
/// looking for.  Knowing we have not met that target allows us to exit
/// early.
///
/// The cost returned is not always the true cost of the matching.
/// If it is equal to target_size or target_size-1, then it is the true cost
/// of the matching;  the idea is that with that knowledge, we can decide
/// whether or not to continue with other matchings.  However, if the size is
/// less than target_size-1, there is no point in continuing to do further
/// matchings later in the algorithm.  So if cost is below that threshold,
/// that is not necessarily reflective of the true maximum flow, but rather a
/// way of signaling that neither this nor subsequent matchings will be useful.
fn max_matching<EK: EdmondsKarp<i32>>(
    u_null: NodeIndex,
    v_null: NodeIndex,
    set_x: &[NodeIndex],
    set_y: &[NodeIndex],
    graph_g: &GraphType,
    graph_h: &GraphType,
    set_s: &mut SetSType,
) {
    // The NodeIndex objects probably share values between set X and set Y
    // since they refer from distinct graphs.  So we need some way of
    // distinguishing them from one another when they are put into the same
    // graph.
    //
    // Strings seem like the easiest solutions - we could prepend an X or Y
    // identifier.  But the resulting cloning and concatenating is relatively
    // expensive.
    //
    // Another option is tuples, but tuples don't work with the Edmonds
    // Karp library.
    //
    // So in the end, the code uses usizes.  The numbers for set X are their
    // original indices + size(graph_h).  This distinguishes them from set Y.
    // The numbers for set Y are just their NodeIndex index values.  And
    // finally, source is size(graph_g) + size(graph_h) + 2, and sink is source + 1


    // TODO:  can probably just remove and add nodes/edges to the vertices/edges
    // vectors instead of making new ones every time - see https://github.com/samueltardieu/pathfinding/blob/b51810f7b4c06f925b044e102efc7a894f8b27a1/tests/edmondskarp.rs#L111
    // for details


    // 1. Set the mappings between the ints and nodes

    // index_to_node is an index, set by i, to (bool, Node)
    // the bool is true if it's part of set X, and false if it's part of set Y
    let mut index_to_node = IndexMap::with_capacity(set_x.len()+set_y.len());
    let mut x_node_to_index = IndexMap::with_capacity(set_x.len());
    let mut y_node_to_index = IndexMap::with_capacity(set_y.len());
    let mut i = 0;
    for node in set_x {
        index_to_node.insert(i, node);
        x_node_to_index.insert(node, i);
        i += 1;
    }
    for node in set_y {
        index_to_node.insert(i, node);
        y_node_to_index.insert(node, i);
        i += 1;
    }
    let source = i+1;
    let sink = i+2;

    // 2. Make the graph
    let mut ek = EK::new(index_to_node.keys().len()+3, source, sink);

    for u in set_x {
        ek.set_capacity(source, x_node_to_index[u], 1); 
    }
    for v in set_y {
        ek.set_capacity(y_node_to_index[v], sink, 1); 
    }

    let mut num_xy_edges = 0;
    for u in set_x {
        for v in set_y {
            if set_s[&SetSKey { val1: *v, val2: *u }].contains_key(&u_null)
                && has_property_subset(
                    &graph_g.node_weight(*v).unwrap().1,
                    &graph_h.node_weight(*u).unwrap().1,
                )
            {
                num_xy_edges += 1;
                ek.set_capacity(x_node_to_index[u], y_node_to_index[v], 1);
            }
        }
    }

    // If even adding one more edge does not get you near the target size,
    // then there is no hope of having a useful matching.  Just return
    if num_xy_edges + 1 < set_x.len() {
        return;
    }

    // 2. Try the mapping with the entirety of X
    let (mapping, cost) = ek.augment();
    if cost as usize == set_x.len() {
        if !set_s[&SetSKey { val1: v_null, val2: u_null }].contains_key(&u_null) {
            // convert mapping from ints to nodes
            let mut nodes_mapping = Vec::new();
            for (map, flow) in mapping {
                if map.0 != source && map.1 != sink && flow != 0 {
                    nodes_mapping.push((*index_to_node[map.0], *index_to_node[map.1]));
                }
            }
            set_s
                .get_mut(&SetSKey { val1: v_null, val2: u_null })
                .unwrap()
                .insert(u_null, Some(nodes_mapping));
        }
    } else if (cost as usize) < set_x.len()-1 {
        return; // you aren't going to get higher flows by taking away edges
    }

    // 3. Try mapping each of the X_i sets;  instead of creating new graphs
    //    entirely, you can just zero out the related edges and recompute
    for x_node in set_x {
        ek.set_capacity(source, x_node_to_index[x_node], 0);
        let (mapping, cost) = ek.augment();
        if cost as usize == set_x.len()-1 {
            if !set_s[&SetSKey { val1: v_null, val2: u_null }].contains_key(x_node) {
                // convert mapping from ints to nodes
                let mut nodes_mapping = Vec::new();
                for (map, flow) in mapping {
                    if map.0 != source && map.1 != sink && flow != 0 {
                        nodes_mapping.push((*index_to_node[map.0], *index_to_node[map.1]));
                    }
                }
                set_s
                    .get_mut(&SetSKey { val1: v_null, val2: u_null })
                    .unwrap()
                    .insert(*x_node, Some(nodes_mapping));
            }
        }
        // set it back the way it was
        ek.set_capacity(source, x_node_to_index[&x_node], 1);
    }
}

// For debugging only
#[allow(dead_code)]
fn print_set_s(
    graph_g: &GraphType,
    graph_h: &GraphType,
    set_s: &SetSType,
) {
    for key in set_s.keys() {
        print!(
            "key: {:?} {:?} ",
            graph_g.node_weight(key.val1).unwrap(),
            graph_h.node_weight(key.val2).unwrap()
        );
        for value_key in set_s[key].keys() {
            print!("inner key: {:?} ", graph_h.node_weight(*value_key).unwrap());
            for mapping in &set_s[key][value_key] {
                for map in mapping {
                    print!(
                        "maps {:?} to {:?} ",
                        graph_h.node_weight(map.0).unwrap(),
                        graph_g.node_weight(map.1).unwrap()
                    );
                }
            }
        }
        print!("\n\n");
    }
}

// Note:  the mapping from graph H to graph G returned by this function may
// map "None" in graph H to things in graph G;  it may also have duplicates
// It is extra work to remove them, and they do not hurt, since we're never
// going to look up what None in graph H maps to, and duplicate matchings
// between the same nodes are clearly not "wrong".  But because of this,
// the size of the matching returned might be a bit wonky.
fn get_mapping_from_set_s(
    graph_h: &GraphType,
    set_s: &SetSType,
    root_in_g: &NodeIndex,
) -> Vec<(NodeIndex, NodeIndex)> {
    let root_h = find_root(graph_h);
    let mut to_return = Vec::with_capacity(graph_h.node_count());
    let mut set_to_find_mapping = vec![(root_h, *root_in_g)];
    while !set_to_find_mapping.is_empty() {
        let key = set_to_find_mapping.pop().unwrap();
        if !to_return.contains(&key) {
            to_return.push(key);
        }

        let set_s_key = SetSKey {
            val1: key.1,
            val2: key.0,
        };
        if set_s[&set_s_key].contains_key(&key.0) {
            if let Some(mapping_vec) = &set_s[&set_s_key][&key.0] {
                for mapping in mapping_vec {
                    if !to_return.contains(&(mapping.1, mapping.0)) {
                        to_return.push((mapping.1, mapping.0));
                        set_to_find_mapping.push(*mapping);
                    }
                }
            }
        }
    }
    to_return
}

fn find_mapping_shamir_inner_loop(
    v: NodeIndex,
    graph_g: &GraphType,
    graph_h: &GraphType,
    set_s: &mut SetSType,
) -> (bool, Option<NodeIndex>) {
    let root_h = find_root(&graph_h);
    let v_neighbors: Vec<NodeIndex> = graph_g.neighbors_undirected(v).collect();
    for u in graph_h.node_indices() {
        let u_neighbors: Vec<NodeIndex> = graph_h.neighbors_undirected(u).collect();
        // all vertices of degree at most t+1
        if u_neighbors.len() > v_neighbors.len() + 1 {
            continue;
        }
        // perform all max matching problems
        max_matching::<DenseCapacity<_>>(
            u, v, &u_neighbors, &v_neighbors, graph_g, graph_h, set_s);

        // lines 12-14 in Shamir and Tsur pseudocode
        if set_s[&SetSKey {
            val1: v,
            val2: root_h,
        }]
            .contains_key(&root_h)
            && has_property_subset(
                &graph_g.node_weight(v).unwrap().1,
                &graph_h.node_weight(root_h).unwrap().1,
            )
        {
            return (true, Some(v));
        }
    }
    // before returning false, we can trim set S
    // TODO:  how to make this jive with finding the actual matching? right now
    // we get rid of non-matching grandchild info in distributed;  this may or
    // may not be impactful
    // we traverse all that is reachable from v;  any that are descendants and
    // not children, ie, grandchildren or later, are removed from set S so
    // set S stays as brief as possible
    /*
    let mut post_order = DfsPostOrder::new(graph_g, v);
    let mut to_remove = Vec::new();
    while let Some(node) = post_order.next(graph_g) {
        if !graph_g.contains_edge(v, node) && v != node {
            to_remove.push(node);
        }
    }
    set_s.retain(|key, _| !to_remove.contains(&key.val1));
    */
    (false, None)
}

// ----------------- Shamir Isomorphism Algorithm Centralized ----------------

// this performs lines 0-4 in the Shamir paper figure 3
fn initialize_s(
    graph_g: &GraphType,
    graph_h: &GraphType,
) -> SetSType {
    let mut s : SetSType =
        IndexMap::with_capacity(graph_g.node_count()*graph_h.node_count());
    for node_g in graph_g.node_indices() {
        for u in graph_h.node_indices() {
            // initialize S entry as empty set
            s.insert(
                SetSKey {
                    val1: node_g,
                    val2: u,
                },
                IndexMap::new(),
            );
        }
    }
    let root_g = find_root(&graph_g);
    let root_h = find_root(&graph_h);
    for leaf_g in find_leaves(root_g, &graph_g) {
        for leaf_h in find_leaves(root_h, &graph_h) {
            s.get_mut(&SetSKey {
                val1: leaf_g,
                val2: leaf_h,
            })
            .unwrap()
            .insert(leaf_h, Some(vec![(leaf_h, leaf_g)]));
            for neighbor in graph_h.neighbors_directed(leaf_h, Incoming) {
                s.get_mut(&SetSKey {
                    val1: leaf_g,
                    val2: leaf_h,
                })
                .unwrap()
                .insert(neighbor, Some(vec![(leaf_h, leaf_g)]));
            }
        }
    }
    s
}

pub fn find_mapping_shamir_centralized(
    graph_g: &GraphType,
    graph_h: &GraphType,
) -> Option<Vec<(NodeIndex, NodeIndex)>> {
    // TODO:  before even dealing with isomorphism, ask if breadth,
    // height, num nodes match up
    if graph_g.node_count() < graph_h.node_count() {
        return None;
    }

    // initialize S with all N(u) sets, lines 1-4
    let mut set_s = initialize_s(graph_g, graph_h);
    let root_g = find_root(graph_g);

    // postorder traversal and filtering of children for degrees, lines 5-8;
    let mut post_order = DfsPostOrder::new(graph_g, root_g);
    while let Some(node) = post_order.next(graph_g) {
        let (mapping_found, mapping_root) =
            find_mapping_shamir_inner_loop(node, graph_g, graph_h, &mut set_s);
        if mapping_found {
            return Some(get_mapping_from_set_s(
                graph_h,
                &set_s,
                &mapping_root.unwrap(),
            ));
        }
    }
    // line 15
    None
}

// ---------------- Shamir Isomorphism Algorithm Decentralized ---------------
fn initialize_s_for_node(
    graph_g: &GraphType,
    graph_h: &GraphType,
    set_s: &mut SetSType,
    node: NodeIndex,
) {
    for u in graph_h.node_indices() {
        // initialize S entry as empty set
        set_s.insert(
            SetSKey {
                val1: node,
                val2: u,
            },
            IndexMap::new(),
        );
    }
    let root_h = find_root(&graph_h);

    // if I am a leaf
    if graph_g.neighbors_directed(node, Outgoing).count() == 0 {
        for leaf_h in find_leaves(root_h, &graph_h) {
            set_s
                .get_mut(&SetSKey {
                    val1: node,
                    val2: leaf_h,
                })
                .unwrap()
                .insert(leaf_h, Some(vec![(leaf_h, node)]));
            for neighbor in graph_h.neighbors_directed(leaf_h, Incoming) {
                set_s
                    .get_mut(&SetSKey {
                        val1: node,
                        val2: leaf_h,
                    })
                    .unwrap()
                    .insert(neighbor, Some(vec![(leaf_h, node)]));
            }
        }
    }
}

pub fn find_mapping_shamir_decentralized(
    graph_g: &GraphType,
    graph_h: &GraphType,
    set_s: &mut SetSType,
    cur_node: NodeIndex, // what node we are in graph_g
    am_root: bool,
) -> Option<Vec<(NodeIndex, NodeIndex)>> {
    // 1. Add yourself (that is, all your entries) to set S
    initialize_s_for_node(graph_g, graph_h, set_s, cur_node);

    // 2. For all your children, run inner loop
    let mut mapping_root_for_children = None;
    let children : Vec<NodeIndex> = graph_g.neighbors_directed(cur_node, Outgoing).collect();
    for child in &children {
        let (mapping_found, mapping_root) =
            find_mapping_shamir_inner_loop(*child, graph_g, graph_h, set_s);
        if !am_root && mapping_found {
            mapping_root_for_children = mapping_root;
        }
    }
    // delete extraneous grandchild information now that you've done inner loop for all children
    // Although this gets rid of extra, useless info, it absolutely blows
    // up the runtime from roughly 30,000 ns to 480,000 ns for reasons
    // I do not understand
    /*
    set_s.retain(|key, value| {
        let am_child = children.contains(&key.val1);
        let am_cur_node = key.val1 == cur_node;
        let valid_subgraph = value.contains_key(&key.val2);
        am_child || am_cur_node || valid_subgraph
    });
    */

    // 2a. If one of your children matched all of graph_h, return that matching
    if let Some(mrc) = mapping_root_for_children {
        return Some(get_mapping_from_set_s(graph_h, &set_s, &mrc));
    }

    // 3. If you are the root, run the inner loop for yourself as well
    if am_root {
        let (mapping_found, mapping_root) =
            find_mapping_shamir_inner_loop(cur_node, graph_g, graph_h, set_s);
        if mapping_found {
            return Some(get_mapping_from_set_s(
                graph_h,
                &set_s,
                &mapping_root.unwrap(),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_utils::get_node_with_id;
    use serde_json;
    use test::Bencher;

    /// --------------- Graph Creation Helper functions -------------------
    fn three_node_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a = graph.add_node(("a".to_string(), IndexMap::new()));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));
        let c = graph.add_node(("c".to_string(), IndexMap::new()));
        graph.add_edge(a, b, ());
        graph.add_edge(a, c, ());
        return graph;
    }

    fn three_node_chain_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a = graph.add_node(("a".to_string(), IndexMap::new()));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));
        let c = graph.add_node(("c".to_string(), IndexMap::new()));
        graph.add_edge(a, b, ());
        graph.add_edge(b, c, ());
        return graph;
    }

    fn two_node_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a = graph.add_node(("a".to_string(), IndexMap::new()));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));

        graph.add_edge(a, b, ());
        return graph;
    }

    fn three_node_graph_with_properties() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [
            (1, "100".to_string()), // height
            (2, "5".to_string()), // breadth
        ]
        .iter()
        .cloned()
        .collect();
        let a = graph.add_node(("a".to_string(), a_hashmap));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));
        let c = graph.add_node(("c".to_string(), IndexMap::new()));
        graph.add_edge(a, b, ());
        graph.add_edge(a, c, ());
        return graph;
    }

    fn two_node_graph_with_properties() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [(1, "100".to_string())]
            .iter()
            .cloned()
            .collect();
        let a = graph.add_node(("a".to_string(), a_hashmap));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));

        graph.add_edge(a, b, ());
        return graph;
    }

    fn two_node_graph_with_wrong_properties() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [(1, "1".to_string())]
            .iter()
            .cloned()
            .collect();
        let a = graph.add_node(("a".to_string(), a_hashmap));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));

        graph.add_edge(a, b, ());
        return graph;
    }

    fn chain_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let a = graph.add_node(("a".to_string(), IndexMap::new()));
        let b = graph.add_node(("b".to_string(), IndexMap::new()));
        let c = graph.add_node(("c".to_string(), IndexMap::new()));
        let star = graph.add_node(("*".to_string(), IndexMap::new()));

        graph.add_edge(a, b, ());
        graph.add_edge(b, c, ());
        graph.add_edge(c, star, ());
        return graph;
    }

    // from figure 2 in shamir paper
    fn g_figure_2() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let r = graph.add_node((String::from("r"), IndexMap::new()));
        let v = graph.add_node((String::from("v"), IndexMap::new()));
        let v1 = graph.add_node((String::from("v1"), IndexMap::new()));
        let v2 = graph.add_node((String::from("v2"), IndexMap::new()));
        let v3 = graph.add_node((String::from("v3"), IndexMap::new()));

        let left_unnamed_child = graph.add_node((String::from("leftchild"), IndexMap::new()));
        let right_unnamed_child = graph.add_node((String::from("rightchild"), IndexMap::new()));

        graph.add_edge(r, v, ());
        graph.add_edge(v, v1, ());
        graph.add_edge(v, v2, ());
        graph.add_edge(v, v3, ());
        graph.add_edge(v1, left_unnamed_child, ());
        graph.add_edge(v1, right_unnamed_child, ());

        return graph;
    }

    // from figure 2 in shamir paper
    fn h_figure_2() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let u = graph.add_node((String::from("u"), IndexMap::new()));
        let u1 = graph.add_node((String::from("u1"), IndexMap::new()));
        let u2 = graph.add_node((String::from("u2"), IndexMap::new()));
        let u3 = graph.add_node((String::from("u3"), IndexMap::new()));
        let u1_left_child = graph.add_node((String::from("u1left"), IndexMap::new()));
        let u1_right_child = graph.add_node((String::from("u1right"), IndexMap::new()));
        let u3_child = graph.add_node((String::from("u3child"), IndexMap::new()));

        graph.add_edge(u, u1, ());
        graph.add_edge(u, u2, ());
        graph.add_edge(u, u3, ());
        graph.add_edge(u1, u1_left_child, ());
        graph.add_edge(u1, u1_right_child, ());
        graph.add_edge(u3, u3_child, ());

        return graph;
    }

    fn three_child_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let root = graph.add_node((String::from("root"), IndexMap::new()));
        let child1 = graph.add_node((String::from("child1"), IndexMap::new()));
        let child2 = graph.add_node((String::from("child2"), IndexMap::new()));
        let child3 = graph.add_node((String::from("child3"), IndexMap::new()));

        graph.add_edge(root, child1, ());
        graph.add_edge(root, child2, ());
        graph.add_edge(root, child3, ());

        return graph;
    }

    fn four_child_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let root = graph.add_node((String::from("root"), IndexMap::new()));
        let child1 = graph.add_node((String::from("child1"), IndexMap::new()));
        let child2 = graph.add_node((String::from("child2"), IndexMap::new()));
        let child3 = graph.add_node((String::from("child3"), IndexMap::new()));
        let child4 = graph.add_node((String::from("child4"), IndexMap::new()));

        graph.add_edge(root, child1, ());
        graph.add_edge(root, child2, ());
        graph.add_edge(root, child3, ());
        graph.add_edge(root, child4, ());

        return graph;
    }

    fn bookinfo_trace_graph() -> GraphType {
        let mut graph : GraphType = Graph::new();
        let productpage = graph.add_node((String::from("productpage-v1"), IndexMap::new()));
        let reviews = graph.add_node((String::from("reviews-v1"), IndexMap::new()));
        let ratings = graph.add_node((String::from("ratings-v1"), IndexMap::new()));
        let details = graph.add_node((String::from("details-v1"), IndexMap::new()));

        graph.add_edge(productpage, reviews, ());
        graph.add_edge(productpage, details, ());
        graph.add_edge(reviews, ratings, ());

        return graph;
    }

    fn simulation_example() -> GraphType {
        let mut graph_g : GraphType = Graph::new();
        let prod_hashmap: IndexMap<u64, String> = [
            (0, "productpage-v1".to_string()), //service name
            (1, "2".to_string()), // height
        ]
        .iter()
        .cloned()
        .collect();
        let prod = graph_g.add_node(("productpage-v1".to_string(), prod_hashmap));

        let ratings_hashmap: IndexMap<u64, String> = [
            (0, "ratings-v1".to_string()), //service name
            (1, "0".to_string()), // height
        ]
        .iter()
        .cloned()
        .collect();
        let ratings = graph_g.add_node(("ratings-v1".to_string(), ratings_hashmap));

        let reviews_hashmap: IndexMap<u64, String> = [
            (0, "reviews-v1".to_string()),
            (1, "1".to_string()), // height
        ]
        .iter()
        .cloned()
        .collect();
        let reviews = graph_g.add_node(("reviews-v1".to_string(), reviews_hashmap));

        let details_hashmap: IndexMap<u64, String> = [
            (0, "details-v1".to_string()),
            (1, "0".to_string()), // height
        ]
        .iter()
        .cloned()
        .collect();
        let details = graph_g.add_node(("details-v1".to_string(), details_hashmap));

        graph_g.add_edge(prod, reviews, ());
        graph_g.add_edge(reviews, ratings, ());
        graph_g.add_edge(prod, details, ());
        return graph_g;
    }

    fn simulation_example_no_properties() -> GraphType {
        let mut graph_g : GraphType = Graph::new();
        let prod = graph_g.add_node(("productpage-v1".to_string(), IndexMap::new()));

        let ratings = graph_g.add_node(("ratings-v1".to_string(), IndexMap::new()));

        let reviews = graph_g.add_node(("reviews-v1".to_string(), IndexMap::new()));

        let details = graph_g.add_node(("details-v1".to_string(), IndexMap::new()));

        graph_g.add_edge(prod, reviews, ());
        graph_g.add_edge(reviews, ratings, ());
        graph_g.add_edge(prod, details, ());
        return graph_g;
    }

    fn biggest_graph() -> GraphType {
        let mut graph_g : GraphType = Graph::new();
        let a = graph_g.add_node(("a".to_string(), IndexMap::new()));
        let b = graph_g.add_node(("b".to_string(), IndexMap::new()));
        let c = graph_g.add_node(("c".to_string(), IndexMap::new()));
        let d = graph_g.add_node(("d".to_string(), IndexMap::new()));
        let e = graph_g.add_node(("e".to_string(), IndexMap::new()));
        let f = graph_g.add_node(("f".to_string(), IndexMap::new()));
        let g = graph_g.add_node(("g".to_string(), IndexMap::new()));

        graph_g.add_edge(a, b, ());
        graph_g.add_edge(b, c, ());
        graph_g.add_edge(c, d, ());
        graph_g.add_edge(d, e, ());
        graph_g.add_edge(d, f, ());
        graph_g.add_edge(d, g, ());
        return graph_g;
    }
    // ---------------------- Shamir Tests -------------------------

    #[test]
    fn test_initialize_s() {
        let graph_g = three_node_graph();
        let graph_h = two_node_graph();
        let s = initialize_s(&graph_g, &graph_h);
        assert!(s.keys().count() == 6);

        // useful debugging if this fails
        for key in s.keys() {
            println!(
                "key: {:?} weight: {:?}, {:?}",
                key,
                graph_g.node_weight(key.val1),
                graph_h.node_weight(key.val2)
            );
        }

        let aa = SetSKey {
            val1: get_node_with_id(&graph_g, "a").unwrap(),
            val2: get_node_with_id(&graph_h, "a").unwrap(),
        };
        let ab = SetSKey {
            val1: get_node_with_id(&graph_g, "a").unwrap(),
            val2: get_node_with_id(&graph_h, "b").unwrap(),
        };

        let ba = SetSKey {
            val1: get_node_with_id(&graph_g, "b").unwrap(),
            val2: get_node_with_id(&graph_h, "a").unwrap(),
        };
        let bb = SetSKey {
            val1: get_node_with_id(&graph_g, "b").unwrap(),
            val2: get_node_with_id(&graph_h, "b").unwrap(),
        };

        let ca = SetSKey {
            val1: get_node_with_id(&graph_g, "c").unwrap(),
            val2: get_node_with_id(&graph_h, "a").unwrap(),
        };
        let cb = SetSKey {
            val1: get_node_with_id(&graph_g, "c").unwrap(),
            val2: get_node_with_id(&graph_h, "b").unwrap(),
        };

        assert!(s.contains_key(&aa));
        assert!(s.contains_key(&ab));

        assert!(s.contains_key(&ba));
        assert!(s.contains_key(&bb));

        assert!(s.contains_key(&ca));
        assert!(s.contains_key(&cb));

        assert!(s[&aa].len() == 0);
        assert!(s[&ba].len() == 0);
        assert!(s[&ca].len() == 0);

        assert!(s[&bb].len() == 2, "bb len is {:?}", s[&bb].len());
        assert!(s[&cb].len() == 2, "cb len is {:?}", s[&cb].len());
    }

    #[test]
    fn test_shamir_small_graphs() {
        let graph_g = three_node_graph();
        let graph_h = two_node_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_some());
    }
    #[test]
    fn test_shamir_figure_2() {
        let graph_g = g_figure_2();
        let graph_h = h_figure_2();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_none());
    }

    #[test]
    fn test_shamir_chain_graphs() {
        let graph_g = chain_graph();
        let graph_h_1 = two_node_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h_1).is_some());
    }

    #[test]
    fn test_shamir_branching_graphs() {
        let graph_g = four_child_graph();
        let graph_h = three_child_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_some());

        let graph_g_2 = three_child_graph();
        let graph_h_2 = four_child_graph();
        assert!(find_mapping_shamir_centralized(&graph_g_2, &graph_h_2).is_none());
    }

    #[test]
    fn test_shamir_on_bookinfo() {
        let graph_g = bookinfo_trace_graph();
        let graph_h = three_node_graph();
        let mapping_wrapped = find_mapping_shamir_centralized(&graph_g, &graph_h);
        assert!(mapping_wrapped.is_some());
        let mapping = mapping_wrapped.unwrap();
        let a = get_node_with_id(&graph_h, "a").unwrap();
        let b = get_node_with_id(&graph_h, "b").unwrap();
        let c = get_node_with_id(&graph_h, "c").unwrap();
        let prod = get_node_with_id(&graph_g, "productpage-v1").unwrap();
        let det = get_node_with_id(&graph_g, "details-v1").unwrap();
        let rev = get_node_with_id(&graph_g, "reviews-v1").unwrap();
        assert!(mapping.contains(&(a, prod)));
        assert!(mapping.contains(&(b, det)) || mapping.contains(&(c, det)));
        assert!(mapping.contains(&(b, rev)) || mapping.contains(&(c, rev)));

        let graph_g_2 = bookinfo_trace_graph();
        let graph_h_2 = three_node_chain_graph();
        let mapping_wrapped_2 = find_mapping_shamir_centralized(&graph_g_2, &graph_h_2);
        assert!(mapping_wrapped_2.is_some());
        let mapping_2 = mapping_wrapped_2.unwrap();
        let a_2 = get_node_with_id(&graph_h_2, "a").unwrap();
        let b_2 = get_node_with_id(&graph_h_2, "b").unwrap();
        let prod_2 = get_node_with_id(&graph_g_2, "productpage-v1").unwrap();
        let rev_2 = get_node_with_id(&graph_g_2, "reviews-v1").unwrap();
        assert!(mapping_2.contains(&(a_2, prod_2)));
        assert!(mapping_2.contains(&(b_2, rev_2)));
    }

    #[test]
    fn test_shamir_full_match() {
        let graph_g = three_node_graph();
        let graph_h = three_node_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_some());
    }

    #[test]
    fn test_property_matches() {
        let graph_g = three_node_graph_with_properties();
        let graph_h = two_node_graph_with_properties();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_some());
        let graph_h_2 = two_node_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h_2).is_some());
    }

    #[test]
    fn test_property_does_not_match() {
        let graph_g = three_node_graph_with_properties();
        let graph_h = two_node_graph_with_wrong_properties();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_none());

        let graph_g_2 = three_node_graph();
        let graph_h_2 = two_node_graph_with_wrong_properties();
        assert!(find_mapping_shamir_centralized(&graph_g_2, &graph_h_2).is_none());
    }

    #[test]
    fn test_simulation_example() {
        let graph_g = simulation_example_no_properties();
        let mut graph_h : GraphType = Graph::new();
        let a = graph_h.add_node(("a".to_string(), IndexMap::new()));
        let b = graph_h.add_node(("b".to_string(), IndexMap::new()));
        graph_h.add_edge(a, b, ());

        let mapping_wrapped = find_mapping_shamir_centralized(&graph_g, &graph_h);
        assert!(mapping_wrapped.is_some());

        let graph_g_2 = simulation_example();
        assert!(find_mapping_shamir_centralized(&graph_g_2, &graph_h).is_some());
    }

    #[test]
    fn test_simulation_example_no_match() {
        let graph_g = simulation_example();

        let mut graph_h : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [(1, "0".to_string())]
            .iter()
            .cloned()
            .collect();
        let a = graph_h.add_node((String::from("productpage-v1"), a_hashmap));
        let b = graph_h.add_node((String::from("reviews-v1"), IndexMap::new()));
        let c = graph_h.add_node((String::from("ratings-v1"), IndexMap::new()));

        graph_h.add_edge(a, b, ());
        graph_h.add_edge(b, c, ());

        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h).is_none());
    }

    #[test]
    fn test_decentralized() {
        let mut set_s: SetSType = IndexMap::new();
        let graph_h = three_node_chain_graph();

        let mut graph_g : GraphType = Graph::new();
        let a = graph_g.add_node((String::from("a"), IndexMap::new()));
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, a, false);
        assert!(ret.is_none());

        let b = graph_g.add_node((String::from("b"), IndexMap::new()));
        graph_g.add_edge(b, a, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, b, false);
        assert!(ret.is_none());

        let c = graph_g.add_node((String::from("c"), IndexMap::new()));
        graph_g.add_edge(c, b, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, c, true);
        assert!(ret.is_some());
    }

    #[test]
    fn test_decentralized_complex() {
        let mut set_s =
            IndexMap::<SetSKey, IndexMap<NodeIndex, Option<Vec<(NodeIndex, NodeIndex)>>>>::new();

        // create graph h
        let mut graph_h : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [(1, "2".to_string())]
            .iter()
            .cloned()
            .collect();
        let a = graph_h.add_node((String::from("productpage-v1"), a_hashmap));
        let b = graph_h.add_node((String::from("reviews-v1"), IndexMap::new()));
        let c = graph_h.add_node((String::from("ratings-v1"), IndexMap::new()));

        graph_h.add_edge(a, b, ());
        graph_h.add_edge(b, c, ());

        //create graph g
        let mut graph_g : GraphType = Graph::new();
        let ratings_hashmap: IndexMap<u64, String> = [
            (0, "ratings-v1".to_string()),
            (1, "0".to_string()),
        ]
        .iter()
        .cloned()
        .collect();
        let ratings = graph_g.add_node(("ratings-v1".to_string(), ratings_hashmap));
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, ratings, false);
        assert!(ret.is_none());

        let reviews_hashmap: IndexMap<u64, String> = [
            (0, "reviews-v1".to_string()),
            (1, "1".to_string()),
        ]
        .iter()
        .cloned()
        .collect();
        let reviews = graph_g.add_node(("reviews-v1".to_string(), reviews_hashmap));
        graph_g.add_edge(reviews, ratings, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, reviews, false);
        assert!(ret.is_none());

        let prod_hashmap: IndexMap<u64, String> = [
            (0, "productpage-v1".to_string()),
            (1, "2".to_string()),
        ]
        .iter()
        .cloned()
        .collect();
        let prod = graph_g.add_node(("productpage-v1".to_string(), prod_hashmap));

        graph_g.add_edge(prod, reviews, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, prod, true);
        assert!(ret.is_some());
    }

    #[test]
    fn test_decentralized_complex_wrong_properties() {
        let mut set_s =
            IndexMap::<SetSKey, IndexMap<NodeIndex, Option<Vec<(NodeIndex, NodeIndex)>>>>::new();

        // create graph h
        let mut graph_h : GraphType = Graph::new();
        let a_hashmap: IndexMap<u64, String> = [(1, "2".to_string())]
            .iter()
            .cloned()
            .collect();
        let a = graph_h.add_node((String::from("productpage-v1"), a_hashmap));
        let b = graph_h.add_node((String::from("reviews-v1"), IndexMap::new()));
        let c = graph_h.add_node((String::from("ratings-v1"), IndexMap::new()));

        graph_h.add_edge(a, b, ());
        graph_h.add_edge(b, c, ());

        //create graph g
        let mut graph_g : GraphType = Graph::new();
        let ratings_hashmap: IndexMap<u64, String> = [
            (0, "ratings-v1".to_string()),
            (1, "0".to_string()), // height
        ]
        .iter()
        .cloned()
        .collect();
        let ratings = graph_g.add_node(("ratings-v1".to_string(), ratings_hashmap));
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, ratings, false);
        assert!(ret.is_none());

        let reviews_hashmap: IndexMap<u64, String> = [
            (0, "reviews-v1".to_string()),
            (1, "1".to_string()), //height
        ]
        .iter()
        .cloned()
        .collect();
        let reviews = graph_g.add_node(("reviews-v1".to_string(), reviews_hashmap));
        graph_g.add_edge(reviews, ratings, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, reviews, false);
        assert!(ret.is_none());

        let prod_hashmap: IndexMap<u64, String> = [
            (1, "0".to_string()), // WRONG PROPERTY, should make this fail, height
            (0, "productpage-v1".to_string()),
        ]
        .iter()
        .cloned()
        .collect();
        let prod = graph_g.add_node(("productpage-v1".to_string(), prod_hashmap));

        graph_g.add_edge(prod, reviews, ());
        let ret = find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, prod, true);
        assert!(ret.is_none());
    }

    #[test]
    fn test_set_s_key_serialization() {
        let set_s_key = SetSKey {
            val1: NodeIndex::new(5),
            val2: NodeIndex::new(10),
        };
        let key_as_str = serde_json::to_string(&set_s_key).unwrap();
        print!("key as str: {:?}", key_as_str);
        let back_to_key: SetSKey = serde_json::from_str(&key_as_str).unwrap();
        assert!(back_to_key.val1 == NodeIndex::new(5));
        assert!(back_to_key.val2 == NodeIndex::new(10));
    }

    #[test]
    fn test_big_graph() {
        let graph_g = biggest_graph();
        let graph_h = three_child_graph();
        let mapping_wrapped = find_mapping_shamir_centralized(&graph_g, &graph_h);
        assert!(mapping_wrapped.is_some());

        let graph_h_not_a_match = four_child_graph();
        assert!(find_mapping_shamir_centralized(&graph_g, &graph_h_not_a_match).is_none());
    }

    #[bench]
    fn bench_centralized(bencher: &mut Bencher) {
        let graph_h = three_node_chain_graph();
        let mut graph_g : GraphType = Graph::new();
        let a = graph_g.add_node((String::from("a"), IndexMap::new()));
        let b = graph_g.add_node((String::from("b"), IndexMap::new()));
        graph_g.add_edge(b, a, ());
        let c = graph_g.add_node((String::from("c"), IndexMap::new()));
        graph_g.add_edge(c, b, ());
        bencher.iter(|| find_mapping_shamir_centralized(&graph_g, &graph_h));
    }

    #[bench]
    fn bench_decentralized(bencher: &mut Bencher) {
        let mut set_s: SetSType = IndexMap::new();
        let graph_h = three_node_chain_graph();
        let mut graph_g : GraphType = Graph::new();

        bencher.iter(|| {
            let a = graph_g.add_node((String::from("a"), IndexMap::new()));
            find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, a, false);

            let b = graph_g.add_node((String::from("b"), IndexMap::new()));
            graph_g.add_edge(b, a, ());
            find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, b, false);

            let c = graph_g.add_node((String::from("c"), IndexMap::new()));
            graph_g.add_edge(c, b, ());
            find_mapping_shamir_decentralized(&graph_g, &graph_h, &mut set_s, c, true);
        });
    }
}
