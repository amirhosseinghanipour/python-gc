use crate::GCResult;
use crate::error::GCError;
use crate::generation::GenerationManager;
use crate::object::{ObjectId, PyObject};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum GCState {
    Reachable,
    Unreachable,
    HasFinalizer,
}

#[derive(Debug)]
pub struct Collector {
    pub generation_manager: GenerationManager,
    pub tracked_objects: HashMap<ObjectId, PyObject>,
    pub collecting_objects: HashSet<ObjectId>,
    pub uncollectable: Vec<PyObject>,
    pub debug_flags: u32,
}

unsafe impl Send for Collector {}
unsafe impl Sync for Collector {}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {
    pub fn new() -> Self {
        Self {
            generation_manager: GenerationManager::new(),
            tracked_objects: HashMap::new(),
            collecting_objects: HashSet::new(),
            uncollectable: Vec::new(),
            debug_flags: 0,
        }
    }

    pub fn track_object(&mut self, mut obj: PyObject) -> GCResult<()> {
        if obj.gc_tracked {
            return Err(GCError::AlreadyTracked);
        }

        obj.gc_head.set_refs(obj.get_refcount() as isize);
        obj.gc_tracked = true;
        let obj_id = obj.id;

        if obj.has_finalizer {
            self.uncollectable.push(obj);
        } else {
            self.tracked_objects.insert(obj_id, obj);
            self.generation_manager.add_to_generation0_fast(obj_id)?;
        }

        Ok(())
    }

    pub fn track_object_fast(&mut self, mut obj: PyObject) -> GCResult<()> {
        if obj.gc_tracked {
            return Err(GCError::AlreadyTracked);
        }

        obj.gc_tracked = true;
        let obj_id = obj.id;

        if obj.has_finalizer {
            self.uncollectable.push(obj);
        } else {
            self.tracked_objects.insert(obj_id, obj);
            self.generation_manager.add_to_generation0_fast(obj_id)?;
        }

        Ok(())
    }

    pub fn track_objects_bulk(&mut self, objects: Vec<PyObject>) -> GCResult<()> {
        let mut count = 0;
        for mut obj in objects {
            if !obj.gc_tracked {
                obj.gc_tracked = true;
                self.tracked_objects.insert(obj.id, obj);
                count += 1;
            }
        }

        self.generation_manager.generations[0].count += count;

        Ok(())
    }

    pub fn untrack_object(&mut self, obj_id: &ObjectId) -> GCResult<()> {
        if !self.tracked_objects.contains_key(obj_id) {
            return Err(GCError::NotTracked);
        }

        self.tracked_objects.remove(obj_id);
        self.generation_manager
            .get_generation_mut(0)
            .ok_or(GCError::Internal("Generation 0 not found".to_string()))?
            .remove_object(obj_id)?;

        Ok(())
    }

    pub fn untrack_object_fast(&mut self, obj_id: &ObjectId) -> GCResult<()> {
        if !self.tracked_objects.contains_key(obj_id) {
            return Err(GCError::NotTracked);
        }

        self.tracked_objects.remove(obj_id);
        Ok(())
    }

    pub fn collect(&mut self) -> GCResult<usize> {
        self.collect_generation(0)
    }

    pub fn collect_fast(&mut self) -> GCResult<usize> {
        if self.tracked_objects.len() < 100 {
            let mut collected = 0;
            let objects_to_collect: Vec<ObjectId> = self.tracked_objects.keys().cloned().collect();

            for obj_id in objects_to_collect {
                if self.untrack_object_fast(&obj_id).is_ok() {
                    collected += 1;
                }
            }

            Ok(collected)
        } else {
            self.collect()
        }
    }

    pub fn collect_generation(&mut self, generation: usize) -> GCResult<usize> {
        if generation >= 3 {
            return Ok(0);
        }

        let mut collected = 0;
        let objects_to_collect: Vec<ObjectId> = self.tracked_objects.keys().cloned().collect();

        for obj_id in objects_to_collect {
            if self.untrack_object_fast(&obj_id).is_ok() {
                collected += 1;
            }
        }

        self.generation_manager.generations[generation].count = 0;

        Ok(collected)
    }

    pub fn get_count(&self) -> usize {
        self.tracked_objects.len()
    }

    pub fn get_stats(&self) -> crate::GCStats {
        crate::GCStats {
            collections: 0,
            collected: 0,
            uncollectable: self.uncollectable.len(),
            total_tracked: self.tracked_objects.len(),
            generation_counts: [
                self.generation_manager.generations[0].count,
                self.generation_manager.generations[1].count,
                self.generation_manager.generations[2].count,
            ],
        }
    }

    pub fn set_debug_flags(&mut self, flags: u32) {
        self.debug_flags = flags;
    }

    pub fn get_debug_flags(&self) -> u32 {
        self.debug_flags
    }
}
