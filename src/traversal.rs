use crate::GCResult;
use crate::error::GCError;
use crate::object::{ObjectId, PyObject};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct Reference {
    pub from: ObjectId,
    pub to: ObjectId,
    pub reference_type: ReferenceType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceType {
    Direct,
    Weak,
    Finalizer,
}

#[derive(Debug)]
pub struct ObjectGraph {
    objects: HashMap<ObjectId, PyObject>,

    references: HashMap<ObjectId, Vec<Reference>>,

    reverse_references: HashMap<ObjectId, Vec<ObjectId>>,
}

impl ObjectGraph {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            references: HashMap::new(),
            reverse_references: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, obj: PyObject) {
        let obj_id = obj.id;
        self.objects.insert(obj_id, obj);
        self.references.insert(obj_id, Vec::new());
        self.reverse_references.insert(obj_id, Vec::new());
    }

    pub fn remove_object(&mut self, obj_id: &ObjectId) -> Option<PyObject> {
        if let Some(refs) = self.reverse_references.remove(obj_id) {
            for from_id in refs {
                if let Some(from_refs) = self.references.get_mut(&from_id) {
                    from_refs.retain(|r| r.to != *obj_id);
                }
            }
        }

        self.references.remove(obj_id);

        self.objects.remove(obj_id)
    }

    pub fn add_reference(
        &mut self,
        from: ObjectId,
        to: ObjectId,
        ref_type: ReferenceType,
    ) -> GCResult<()> {
        if !self.objects.contains_key(&from) || !self.objects.contains_key(&to) {
            return Err(GCError::Internal("Object not found in graph".to_string()));
        }

        let reference = Reference {
            from,
            to,
            reference_type: ref_type,
        };

        self.references.entry(from).or_default().push(reference);

        self.reverse_references.entry(to).or_default().push(from);

        Ok(())
    }

    pub fn remove_reference(&mut self, from: ObjectId, to: ObjectId) -> GCResult<()> {
        if let Some(refs) = self.references.get_mut(&from) {
            refs.retain(|r| r.to != to);
        }

        if let Some(reverse_refs) = self.reverse_references.get_mut(&to) {
            reverse_refs.retain(|&id| id != from);
        }

        Ok(())
    }

    pub fn get_referrers(&self, obj_id: &ObjectId) -> Vec<&PyObject> {
        self.reverse_references
            .get(obj_id)
            .map(|refs| refs.iter().filter_map(|id| self.objects.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn get_references(&self, obj_id: &ObjectId) -> Vec<&PyObject> {
        self.references
            .get(obj_id)
            .map(|refs| {
                refs.iter()
                    .filter_map(|r| self.objects.get(&r.to))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn find_reachable(&self, roots: &[ObjectId]) -> HashSet<ObjectId> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        for root_id in roots {
            reachable.insert(*root_id);
            queue.push_back(*root_id);
        }

        while let Some(current_id) = queue.pop_front() {
            if let Some(refs) = self.references.get(&current_id) {
                for reference in refs {
                    if !reachable.contains(&reference.to) {
                        reachable.insert(reference.to);
                        queue.push_back(reference.to);
                    }
                }
            }
        }

        reachable
    }

    pub fn find_unreachable(&self, roots: &[ObjectId]) -> HashSet<ObjectId> {
        let reachable = self.find_reachable(roots);
        let all_objects: HashSet<ObjectId> = self.objects.keys().copied().collect();

        all_objects.difference(&reachable).copied().collect()
    }

    pub fn detect_cycles(&self) -> Vec<Vec<ObjectId>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for obj_id in self.objects.keys() {
            if !visited.contains(obj_id) {
                let mut path = Vec::new();
                self.dfs_cycle_detection(
                    *obj_id,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle_detection(
        &self,
        current_id: ObjectId,
        visited: &mut HashSet<ObjectId>,
        rec_stack: &mut HashSet<ObjectId>,
        path: &mut Vec<ObjectId>,
        cycles: &mut Vec<Vec<ObjectId>>,
    ) {
        visited.insert(current_id);
        rec_stack.insert(current_id);
        path.push(current_id);

        if let Some(refs) = self.references.get(&current_id) {
            for reference in refs {
                let next_id = reference.to;

                if !visited.contains(&next_id) {
                    self.dfs_cycle_detection(next_id, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(&next_id) {
                    if let Some(cycle_start) = path.iter().position(|&id| id == next_id) {
                        let cycle: Vec<ObjectId> = path[cycle_start..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        rec_stack.remove(&current_id);
        path.pop();
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn reference_count(&self) -> usize {
        self.references.values().map(|refs| refs.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn clear(&mut self) {
        self.objects.clear();
        self.references.clear();
        self.reverse_references.clear();
    }

    pub fn get_object(&self, obj_id: &ObjectId) -> Option<&PyObject> {
        self.objects.get(obj_id)
    }

    pub fn get_object_mut(&mut self, obj_id: &ObjectId) -> Option<&mut PyObject> {
        self.objects.get_mut(obj_id)
    }

    pub fn get_all_objects(&self) -> &HashMap<ObjectId, PyObject> {
        &self.objects
    }
}

impl Default for ObjectGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{ObjectData, PyObject};

    #[test]
    fn test_object_graph_creation() {
        let graph = ObjectGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.object_count(), 0);
    }

    #[test]
    fn test_add_remove_object() {
        let mut graph = ObjectGraph::new();

        let obj = PyObject::new("test".to_string(), ObjectData::Integer(42));
        let obj_id = obj.id;

        graph.add_object(obj);
        assert_eq!(graph.object_count(), 1);
        assert!(graph.get_object(&obj_id).is_some());

        let removed = graph.remove_object(&obj_id);
        assert!(removed.is_some());
        assert!(graph.is_empty());
    }

    #[test]
    fn test_add_reference() {
        let mut graph = ObjectGraph::new();

        let obj1 = PyObject::new("obj1".to_string(), ObjectData::Integer(1));
        let obj2 = PyObject::new("obj2".to_string(), ObjectData::Integer(2));

        let id1 = obj1.id;
        let id2 = obj2.id;

        graph.add_object(obj1);
        graph.add_object(obj2);

        assert!(graph.add_reference(id1, id2, ReferenceType::Direct).is_ok());
        assert_eq!(graph.reference_count(), 1);

        let referrers = graph.get_referrers(&id2);
        assert_eq!(referrers.len(), 1);
        assert_eq!(referrers[0].id, id1);
    }

    #[test]
    fn test_find_reachable() {
        let mut graph = ObjectGraph::new();

        let obj1 = PyObject::new("obj1".to_string(), ObjectData::Integer(1));
        let obj2 = PyObject::new("obj2".to_string(), ObjectData::Integer(2));
        let obj3 = PyObject::new("obj3".to_string(), ObjectData::Integer(3));

        let id1 = obj1.id;
        let id2 = obj2.id;
        let id3 = obj3.id;

        graph.add_object(obj1);
        graph.add_object(obj2);
        graph.add_object(obj3);

        graph
            .add_reference(id1, id2, ReferenceType::Direct)
            .unwrap();
        graph
            .add_reference(id2, id3, ReferenceType::Direct)
            .unwrap();

        let reachable = graph.find_reachable(&[id1]);
        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains(&id1));
        assert!(reachable.contains(&id2));
        assert!(reachable.contains(&id3));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = ObjectGraph::new();

        let obj1 = PyObject::new("obj1".to_string(), ObjectData::Integer(1));
        let obj2 = PyObject::new("obj2".to_string(), ObjectData::Integer(2));

        let id1 = obj1.id;
        let id2 = obj2.id;

        graph.add_object(obj1);
        graph.add_object(obj2);

        graph
            .add_reference(id1, id2, ReferenceType::Direct)
            .unwrap();
        graph
            .add_reference(id2, id1, ReferenceType::Direct)
            .unwrap();

        let cycles = graph.detect_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 2);
    }
}
