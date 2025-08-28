use crate::GCResult;
use crate::error::GCError;
use crate::generation::GenerationManager;
use crate::object::{ObjectId, PyGCHead, PyObject};
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

        let mut gc_head = PyGCHead::new();
        gc_head.set_refs(obj.get_refcount() as isize);

        self.generation_manager.add_to_generation0(obj.clone())?;

        obj.gc_tracked = true;
        obj.gc_head = Some(gc_head);

        self.tracked_objects.insert(obj.id, obj);

        Ok(())
    }

    pub fn track_object_fast(&mut self, mut obj: PyObject) -> GCResult<()> {
        if obj.gc_tracked {
            return Err(GCError::AlreadyTracked);
        }

        obj.gc_tracked = true;
        obj.gc_head = Some(PyGCHead::new());

        self.generation_manager.add_to_generation0(obj.clone())?;
        self.tracked_objects.insert(obj.id, obj);

        Ok(())
    }

    pub fn untrack_object(&mut self, obj_id: &ObjectId) -> GCResult<()> {
        let _obj = self
            .tracked_objects
            .remove(obj_id)
            .ok_or(GCError::NotTracked)?;

        self.generation_manager
            .get_generation_mut(0)
            .ok_or(GCError::Internal("Generation 0 not found".to_string()))?
            .remove_object(obj_id)?;

        Ok(())
    }

    pub fn untrack_object_fast(&mut self, obj_id: &ObjectId) -> GCResult<()> {
        let _obj = self
            .tracked_objects
            .remove(obj_id)
            .ok_or(GCError::NotTracked)?;

        if let Some(generation) = self.generation_manager.get_generation_mut(0) {
            let _ = generation.remove_object(obj_id);
        }

        Ok(())
    }

    pub fn collect_generation(&mut self, generation: usize) -> GCResult<usize> {
        if self.collecting_objects.contains(&ObjectId::new()) {
            return Err(GCError::CollectionInProgress);
        }

        self.generation_manager.start_collection(generation)?;

        let _gen = self
            .generation_manager
            .get_generation(generation)
            .ok_or(GCError::Internal(format!(
                "Generation {generation} not found"
            )))?;

        self.merge_younger_generations(generation)?;

        let collected = self.collect_main(generation)?;

        if generation < 2 {
            self.generation_manager.promote_generation(generation)?;
        }

        self.generation_manager.end_collection();

        Ok(collected)
    }

    fn collect_main(&mut self, generation: usize) -> GCResult<usize> {
        let generation_data = self
            .generation_manager
            .get_generation_mut(generation)
            .ok_or(GCError::Internal(format!(
                "Generation {generation} not found"
            )))?;

        if generation_data.is_empty() {
            return Ok(0);
        }

        self.update_refs(generation)?;

        self.subtract_refs(generation)?;

        let unreachable = self.move_unreachable(generation)?;

        let _finalizers = self.handle_finalizers(&unreachable)?;

        let collected = self.clear_unreachable(&unreachable)?;

        self.restore_refs(generation)?;

        Ok(collected)
    }

    fn update_refs(&mut self, generation: usize) -> GCResult<()> {
        let generation_data = self
            .generation_manager
            .get_generation_mut(generation)
            .ok_or(GCError::Internal(format!(
                "Generation {generation} not found"
            )))?;

        let updates: Vec<(ObjectId, usize)> = generation_data
            .get_objects()
            .iter()
            .map(|(id, obj)| (*id, obj.get_refcount()))
            .collect();

        for (obj_id, refcount) in updates {
            if let Some(obj) = generation_data.get_objects_mut().get_mut(&obj_id) {
                if let Some(ref mut gc_head) = obj.gc_head {
                    gc_head.set_refs(refcount as isize);
                    gc_head.set_collecting();
                }
            }
        }

        Ok(())
    }

    fn subtract_refs(&mut self, generation: usize) -> GCResult<()> {
        let generation_data = self
            .generation_manager
            .get_generation_mut(generation)
            .ok_or(GCError::Internal(format!(
                "Generation {generation} not found"
            )))?;

        let obj_ids: Vec<ObjectId> = generation_data.get_objects().keys().copied().collect();

        for obj_id in obj_ids {
            if let Some(obj) = generation_data.get_objects_mut().get_mut(&obj_id) {
                if let Some(ref mut gc_head) = obj.gc_head {
                    let current_refs = gc_head.get_refs();
                    if current_refs > 0 {
                        gc_head.set_refs(current_refs - 1);
                    }
                }
            }
        }

        Ok(())
    }

    fn move_unreachable(&mut self, generation: usize) -> GCResult<Vec<PyObject>> {
        let generation_data =
            self.generation_manager
                .get_generation(generation)
                .ok_or(GCError::Internal(format!(
                    "Generation {generation} not found"
                )))?;

        let mut unreachable = Vec::new();

        for obj in generation_data.get_objects().values() {
            if let Some(ref gc_head) = obj.gc_head {
                if gc_head.get_refs() == 0 {
                    unreachable.push(obj.clone());
                }
            }
        }

        Ok(unreachable)
    }

    fn handle_finalizers(&mut self, unreachable: &[PyObject]) -> GCResult<Vec<PyObject>> {
        let mut finalizers = Vec::new();

        for obj in unreachable {
            if self.has_finalizer(obj) {
                finalizers.push(obj.clone());
            }
        }

        Ok(finalizers)
    }

    fn has_finalizer(&self, obj: &PyObject) -> bool {
        matches!(
            &*obj.data.try_read().unwrap(),
            crate::object::ObjectData::Custom(_)
        )
    }

    fn clear_unreachable(&mut self, unreachable: &[PyObject]) -> GCResult<usize> {
        let mut collected = 0;

        for obj in unreachable {
            if !self.has_finalizer(obj) {
                self.tracked_objects.remove(&obj.id);
                collected += 1;
            } else {
                self.uncollectable.push(obj.clone());
            }
        }

        Ok(collected)
    }

    fn restore_refs(&mut self, generation: usize) -> GCResult<()> {
        let generation_data = self
            .generation_manager
            .get_generation_mut(generation)
            .ok_or(GCError::Internal(format!(
                "Generation {generation} not found"
            )))?;

        let obj_ids: Vec<ObjectId> = generation_data.get_objects().keys().copied().collect();

        for obj_id in obj_ids {
            if let Some(obj) = generation_data.get_objects_mut().get_mut(&obj_id) {
                if let Some(ref mut gc_head) = obj.gc_head {
                    gc_head.clear_collecting();
                }
            }
        }

        Ok(())
    }

    fn merge_younger_generations(&mut self, generation: usize) -> GCResult<()> {
        for i in 0..generation {
            let younger_gen = self
                .generation_manager
                .get_generation_mut(i)
                .ok_or(GCError::Internal(format!("Generation {i} not found")))?;

            let objects = younger_gen.clear();
            for obj in objects {
                self.generation_manager
                    .get_generation_mut(generation)
                    .ok_or(GCError::Internal(format!(
                        "Generation {generation} not found"
                    )))?
                    .add_object(obj)?;
            }
        }

        Ok(())
    }

    pub fn get_stats(&self) -> GCStats {
        GCStats {
            total_tracked: self.tracked_objects.len(),
            generation_counts: [
                self.generation_manager
                    .get_generation(0)
                    .map(|g| g.count)
                    .unwrap_or(0),
                self.generation_manager
                    .get_generation(1)
                    .map(|g| g.count)
                    .unwrap_or(0),
                self.generation_manager
                    .get_generation(2)
                    .map(|g| g.count)
                    .unwrap_or(0),
            ],
            uncollectable: self.uncollectable.len(),
        }
    }

    pub fn set_debug(&mut self, flags: u32) {
        self.debug_flags = flags;
    }

    #[cfg(target_arch = "x86_64")]
    pub fn bulk_collect_objects_simd(&mut self, objects: &[PyObject]) -> usize {
        use std::arch::x86_64::*;

        let mut collected = 0;
        let chunk_size = 8;

        for chunk in objects.chunks(chunk_size) {
            let mut mask = 0u8;

            for (i, obj) in chunk.iter().enumerate() {
                if !obj.gc_tracked || obj.get_refcount() == 0 {
                    mask |= 1 << i;
                }
            }

            for (i, obj) in chunk.iter().enumerate() {
                if (mask & (1 << i)) != 0 {
                    if let Ok(_) = self.untrack_object_fast(&obj.id) {
                        collected += 1;
                    }
                }
            }
        }

        collected
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn bulk_collect_objects_simd(&mut self, objects: &[PyObject]) -> usize {
        self.bulk_collect_objects_standard(objects)
    }

    pub fn bulk_collect_objects_standard(&mut self, objects: &[PyObject]) -> usize {
        let mut collected = 0;

        for obj in objects {
            if !obj.gc_tracked || obj.get_refcount() == 0 {
                if let Ok(_) = self.untrack_object_fast(&obj.id) {
                    collected += 1;
                }
            }
        }

        collected
    }
}

#[derive(Debug, Clone)]
pub struct GCStats {
    pub total_tracked: usize,
    pub generation_counts: [usize; 3],
    pub uncollectable: usize,
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
