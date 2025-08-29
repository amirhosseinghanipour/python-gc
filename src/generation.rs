use crate::GCResult;
use crate::object::{ObjectId, PyGCHead, PyObject};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Generation {
    pub head: PyGCHead,
    pub threshold: usize,
    pub count: usize,
    pub objects: HashMap<ObjectId, PyObject>,
}

impl Generation {
    pub fn new(threshold: usize) -> Self {
        let mut head = PyGCHead::new();
        head._gc_next = &head as *const _ as usize;
        head._gc_prev = &head as *const _ as usize;

        Self {
            head,
            threshold,
            count: 0,
            objects: HashMap::new(),
        }
    }

    pub fn should_collect(&self) -> bool {
        self.count >= self.threshold
    }

    pub fn add_object(&mut self, obj: PyObject) -> GCResult<()> {
        let obj_id = obj.id;

        if self.objects.contains_key(&obj_id) {
            return Err(crate::error::GCError::AlreadyTracked);
        }

        self.insert_into_list(&obj);
        self.objects.insert(obj_id, obj);
        self.count += 1;

        Ok(())
    }

    pub fn remove_object(&mut self, obj_id: &ObjectId) -> GCResult<PyObject> {
        let obj = self
            .objects
            .remove(obj_id)
            .ok_or(crate::error::GCError::NotTracked)?;

        self.remove_from_list(&obj);
        self.count -= 1;
        Ok(obj)
    }

    pub fn get_objects(&self) -> &HashMap<ObjectId, PyObject> {
        &self.objects
    }

    pub fn get_objects_mut(&mut self) -> &mut HashMap<ObjectId, PyObject> {
        &mut self.objects
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn clear(&mut self) -> Vec<PyObject> {
        let objects: Vec<PyObject> = self.objects.drain().map(|(_, obj)| obj).collect();
        self.count = 0;

        self.head._gc_next = &self.head as *const _ as usize;
        self.head._gc_prev = &self.head as *const _ as usize;

        objects
    }

    fn insert_into_list(&mut self, _obj: &PyObject) {}

    fn remove_from_list(&mut self, _obj: &PyObject) {}
}

#[derive(Debug)]
pub struct GenerationManager {
    pub generations: [Generation; 3],
    pub permanent_generation: Generation,
    pub collecting_generation: Option<usize>,
}

impl GenerationManager {
    pub fn new() -> Self {
        Self {
            generations: [
                Generation::new(700),
                Generation::new(10),
                Generation::new(10),
            ],
            permanent_generation: Generation::new(0),
            collecting_generation: None,
        }
    }

    pub fn get_generation(&self, generation_idx: usize) -> Option<&Generation> {
        self.generations.get(generation_idx)
    }

    pub fn get_generation_mut(&mut self, generation_idx: usize) -> Option<&mut Generation> {
        self.generations.get_mut(generation_idx)
    }

    pub fn add_to_generation0(&mut self, obj: PyObject) -> GCResult<()> {
        self.generations[0].add_object(obj)
    }

    pub fn add_to_generation0_fast(&mut self, _obj_id: ObjectId) -> GCResult<()> {
        self.generations[0].count += 1;
        Ok(())
    }

    pub fn bulk_add_to_generation0(&mut self, objects: Vec<PyObject>) -> GCResult<()> {
        let generation = &mut self.generations[0];

        for obj in objects {
            if !generation.objects.contains_key(&obj.id) {
                generation.objects.insert(obj.id, obj);
                generation.count += 1;
            }
        }

        Ok(())
    }

    pub fn promote_generation(&mut self, from_gen: usize) -> GCResult<()> {
        if from_gen >= self.generations.len() - 1 {
            return Ok(());
        }

        let objects = {
            let from_gen_ref = &mut self.generations[from_gen];
            from_gen_ref.clear()
        };

        let to_gen = &mut self.generations[from_gen + 1];
        for obj in objects {
            to_gen.add_object(obj)?;
        }

        Ok(())
    }

    pub fn needs_collection(&self) -> Option<usize> {
        for (i, generation) in self.generations.iter().enumerate() {
            if generation.should_collect() {
                return Some(i);
            }
        }
        None
    }

    pub fn start_collection(&mut self, generation_idx: usize) -> GCResult<()> {
        if self.collecting_generation.is_some() {
            return Err(crate::error::GCError::CollectionInProgress);
        }

        if generation_idx >= self.generations.len() {
            return Err(crate::error::GCError::Internal(format!(
                "Invalid generation: {generation_idx}"
            )));
        }

        self.collecting_generation = Some(generation_idx);
        Ok(())
    }

    pub fn end_collection(&mut self) {
        self.collecting_generation = None;
    }

    pub fn total_objects(&self) -> usize {
        self.generations.iter().map(|g| g.count).sum()
    }
}

impl Default for GenerationManager {
    fn default() -> Self {
        Self::new()
    }
}
