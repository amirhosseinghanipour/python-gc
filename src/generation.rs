use crate::GCResult;
use crate::error::GCError;
use crate::object::{ObjectId, PyObject};

#[derive(Debug)]
pub struct Generation {
    pub count: usize,
    pub threshold: usize,
    pub head: crate::object::PyGCHead,
}

impl Generation {
    pub fn new(threshold: usize) -> Self {
        let mut head = crate::object::PyGCHead::new();
        let head_ptr = &mut head as *mut crate::object::PyGCHead;
        head.set_next(head_ptr);
        head.set_prev(head_ptr);

        Self {
            count: 0,
            threshold,
            head,
        }
    }

    pub fn add_object(&mut self, _obj: PyObject) -> GCResult<()> {
        self.count += 1;
        Ok(())
    }

    pub fn add_object_fast(&mut self, _obj_id: ObjectId) -> GCResult<()> {
        self.count += 1;
        Ok(())
    }

    pub fn remove_object(&mut self, _obj_id: &ObjectId) -> GCResult<()> {
        if self.count > 0 {
            self.count -= 1;
        }
        Ok(())
    }

    pub fn should_collect(&self) -> bool {
        self.count >= self.threshold
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

#[derive(Debug)]
pub struct GenerationManager {
    pub generations: Vec<Generation>,
}

impl Default for GenerationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerationManager {
    pub fn new() -> Self {
        let generations = vec![
            Generation::new(700),
            Generation::new(10),
            Generation::new(10),
        ];

        Self { generations }
    }

    pub fn add_to_generation0(&mut self, obj: PyObject) -> GCResult<()> {
        if let Some(generation) = self.generations.get_mut(0) {
            generation.add_object(obj)
        } else {
            Err(GCError::Internal("Generation 0 not found".to_string()))
        }
    }

    pub fn add_to_generation0_fast(&mut self, obj_id: ObjectId) -> GCResult<()> {
        if let Some(generation) = self.generations.get_mut(0) {
            generation.add_object_fast(obj_id)
        } else {
            Err(GCError::Internal("Generation 0 not found".to_string()))
        }
    }

    pub fn promote_generation(&mut self, from_gen: usize, to_gen: usize) -> GCResult<()> {
        if from_gen >= self.generations.len() || to_gen >= self.generations.len() {
            return Err(GCError::Internal("Invalid generation index".to_string()));
        }

        let from_count = self.generations[from_gen].count;
        self.generations[from_gen].clear();
        self.generations[to_gen].count += from_count;

        Ok(())
    }

    pub fn get_generation(&self, index: usize) -> Option<&Generation> {
        self.generations.get(index)
    }

    pub fn get_generation_mut(&mut self, index: usize) -> Option<&mut Generation> {
        self.generations.get_mut(index)
    }

    pub fn get_total_count(&self) -> usize {
        self.generations.iter().map(|g| g.count).sum()
    }

    pub fn should_collect_generation(&self, generation: usize) -> bool {
        self.generations
            .get(generation)
            .map(|g| g.should_collect())
            .unwrap_or(false)
    }
}
