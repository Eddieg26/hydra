use fixedbitset::FixedBitSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclicDependency(pub Vec<usize>);
pub struct IndexDag<N> {
    nodes: Vec<N>,
    dependents: Vec<FixedBitSet>,
    dependencies: Vec<usize>,
    topology: Vec<usize>,
    is_dirty: bool,
}

impl<N> IndexDag<N> {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            dependents: vec![],
            dependencies: vec![],
            topology: vec![],
            is_dirty: false,
        }
    }

    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut [N] {
        &mut self.nodes
    }

    pub fn topology(&self) -> &[usize] {
        &self.topology
    }

    pub fn dependents(&self) -> &[FixedBitSet] {
        &self.dependents
    }

    pub fn dependencies(&self) -> &[usize] {
        &self.dependencies
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn add_node(&mut self, node: N) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.dependencies.push(0);
        self.dependents.push(FixedBitSet::with_capacity(index));
        self.is_dirty = true;

        index
    }

    pub fn add_dependency(&mut self, dependency: usize, index: usize) {
        self.dependencies[index] += 1;
        self.dependents[dependency].grow(index + 1);
        self.dependents[dependency].set(index, true);
        self.is_dirty = true;
    }

    pub fn remove_dependency(&mut self, dependency: usize, index: usize) -> bool {
        if index < self.dependents[dependency].len() && self.dependents[dependency][index] {
            self.dependents[dependency].set(index, false);
            self.dependencies[index] -= 1;
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    pub fn map<M>(mut self, mut mapper: impl FnMut(N) -> M) -> IndexDag<M> {
        let nodes = self.nodes.drain(..).map(|n| mapper(n)).collect();

        IndexDag {
            nodes,
            dependents: self.dependents,
            dependencies: self.dependencies,
            topology: self.topology,
            is_dirty: self.is_dirty,
        }
    }

    pub fn build(&mut self) -> Result<&[usize], CyclicDependency> {
        if self.is_dirty {
            let mut order = vec![];
            let mut visited = vec![false; self.nodes.len()];
            let mut recursion_stack = vec![false; self.nodes.len()];

            fn visit(
                index: usize,
                dependents: &Vec<FixedBitSet>,
                visited: &mut Vec<bool>,
                recursion_stack: &mut Vec<bool>,
                order: &mut Vec<usize>,
            ) -> Result<(), Vec<usize>> {
                if recursion_stack[index] {
                    return Err(vec![index]);
                }

                if visited[index] {
                    return Ok(());
                }

                visited[index] = true;
                recursion_stack[index] = true;

                for dependent in dependents[index].ones() {
                    if let Err(mut cycle) =
                        visit(dependent, dependents, visited, recursion_stack, order)
                    {
                        cycle.push(index);
                        return Err(cycle);
                    }
                }

                recursion_stack[index] = false;
                order.push(index);
                Ok(())
            }

            for index in 0..self.nodes.len() {
                if !visited[index] {
                    if let Err(mut cycle) = visit(
                        index,
                        &self.dependents,
                        &mut visited,
                        &mut recursion_stack,
                        &mut order,
                    ) {
                        cycle.reverse();
                        return Err(CyclicDependency(cycle));
                    }
                }
            }

            order.reverse();
            self.topology = order;
        }

        Ok(&self.topology)
    }

    pub fn into_immutable(self) -> ImmutableIndexDag<N> {
        ImmutableIndexDag {
            nodes: self.nodes.into_boxed_slice(),
            dependents: self.dependents.into_boxed_slice(),
            dependencies: self.dependencies.into_boxed_slice(),
            topology: self.topology.into_boxed_slice(),
        }
    }

    pub fn into_values(self) -> DagValues<N> {
        DagValues {
            nodes: self.nodes,
            dependents: self.dependents,
            dependencies: self.dependencies,
            topology: self.topology,
        }
    }
}

impl<N> Default for IndexDag<N> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DagValues<N> {
    pub nodes: Vec<N>,
    pub dependents: Vec<FixedBitSet>,
    pub dependencies: Vec<usize>,
    pub topology: Vec<usize>,
}

pub struct ImmutableIndexDag<N> {
    nodes: Box<[N]>,
    dependents: Box<[FixedBitSet]>,
    dependencies: Box<[usize]>,
    topology: Box<[usize]>,
}

impl<N> ImmutableIndexDag<N> {
    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut [N] {
        &mut self.nodes
    }

    pub fn dependents(&self) -> &[FixedBitSet] {
        &self.dependents
    }

    pub fn dependencies(&self) -> &[usize] {
        &self.dependencies
    }

    pub fn topology(&self) -> &[usize] {
        &self.topology
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.topology.iter().map(|i| &self.nodes[*i])
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

mod tests {
    #[test]
    fn add_node() {
        let mut dag = super::IndexDag::new();
        let index = dag.add_node("Node1");
        assert_eq!(index, 0);
        assert_eq!(dag.nodes().len(), 1);
        assert_eq!(dag.nodes()[0], "Node1");
    }

    #[test]
    fn add_dependency() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");

        dag.add_dependency(node1, node2); // Node2 depends on Node1

        assert_eq!(dag.dependencies()[node2], 1);
        assert!(dag.dependents()[node1].contains(node2));
    }

    #[test]
    fn remove_dependency() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");

        dag.add_dependency(node1, node2);
        assert!(dag.remove_dependency(node1, node2));
        assert_eq!(dag.dependencies()[node2], 0);
        assert!(!dag.dependents()[node1].contains(node2));
    }

    #[test]
    fn topological_sort() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");
        let node3 = dag.add_node("Node3");

        dag.add_dependency(node2, node3); // Node3 depends on Node2
        dag.add_dependency(node2, node1); // Node1 depends on Node2

        let result = dag.build();
        assert!(result.is_ok());
        let topology = result.unwrap();
        assert_eq!(topology, &[node2, node3, node1]);
    }

    #[test]
    fn cycle_detection() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");
        let node3 = dag.add_node("Node3");

        dag.add_dependency(node1, node2); // Node2 depends on Node1
        dag.add_dependency(node2, node3); // Node3 depends on Node2
        dag.add_dependency(node3, node1); // Node1 depends on Node3 (creates a cycle)

        let result = dag.build();
        assert!(result.is_err());
        let cycle = result.unwrap_err();
        assert!(cycle.0.contains(&node1));
        assert!(cycle.0.contains(&node2));
        assert!(cycle.0.contains(&node3));
    }

    #[test]
    fn no_dependencies() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");

        let result = dag.build();
        assert!(result.is_ok());
        let topology = result.unwrap();
        assert!(topology.contains(&node1));
        assert!(topology.contains(&node2));
    }

    #[test]
    fn multiple_dependencies() {
        let mut dag = super::IndexDag::new();
        let node1 = dag.add_node("Node1");
        let node2 = dag.add_node("Node2");
        let node3 = dag.add_node("Node3");

        dag.add_dependency(node1, node2); // Node2 depends on Node1
        dag.add_dependency(node1, node3); // Node3 depends on Node1

        let result = dag.build();
        assert!(result.is_ok());
        let topology = result.unwrap();
        assert!(
            topology.iter().position(|&x| x == node1).unwrap()
                < topology.iter().position(|&x| x == node2).unwrap()
        );
        assert!(
            topology.iter().position(|&x| x == node1).unwrap()
                < topology.iter().position(|&x| x == node3).unwrap()
        );
    }
}
