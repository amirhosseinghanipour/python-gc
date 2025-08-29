use crate::GCResult;
use crate::collector::Collector;
use crate::error::GCError;
use crate::object::{ObjectId, PyObject};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug)]
pub struct GarbageCollector {
    collector: Arc<RwLock<Collector>>,
    enabled: bool,
    thresholds: [usize; 3],
    debug_flags: u32,
}

unsafe impl Send for GarbageCollector {}
unsafe impl Sync for GarbageCollector {}

impl GarbageCollector {
    pub fn new() -> Self {
        Self {
            collector: Arc::new(RwLock::new(Collector::new())),
            enabled: true,
            thresholds: [700, 10, 10],
            debug_flags: 0,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn track(&mut self, obj: PyObject) -> GCResult<()> {
        if !self.enabled {
            return Ok(());
        }

        {
            let mut collector = self.collector.write();
            collector.track_object_fast(obj)
        }
    }

    pub fn track_bulk(&mut self, objects: Vec<PyObject>) -> GCResult<()> {
        if !self.enabled {
            return Ok(());
        }

        {
            let mut collector = self.collector.write();
            collector.track_objects_bulk(objects)
        }
    }

    pub fn untrack(&mut self, obj_id: &ObjectId) -> GCResult<()> {
        if !self.enabled {
            return Ok(());
        }

        {
            let mut collector = self.collector.write();
            collector.untrack_object_fast(obj_id)
        }
    }

    pub fn collect_generation(&self, generation: usize) -> GCResult<usize> {
        if !self.enabled {
            return Ok(0);
        }

        let mut collector = self.collector.write();
        collector.collect_generation(generation)
    }

    pub fn collect(&self) -> GCResult<usize> {
        if !self.enabled {
            return Ok(0);
        }

        let mut collector = self.collector.write();
        collector.collect_generation(2)
    }

    pub fn needs_collection(&self) -> bool {
        let collector = self.collector.read();
        collector.generation_manager.should_collect_generation(0)
    }

    pub fn get_stats(&self) -> crate::GCStats {
        let collector = self.collector.read();
        collector.get_stats()
    }

    pub fn set_debug(&mut self, flags: u32) {
        self.debug_flags = flags;
        let mut collector = self.collector.write();
        collector.set_debug_flags(flags);
    }

    pub fn get_debug(&self) -> u32 {
        self.debug_flags
    }

    pub fn get_count(&self) -> usize {
        let collector = self.collector.read();
        collector.get_count()
    }

    pub fn get_generation_count(&self, generation: usize) -> Option<usize> {
        if generation >= 3 {
            return None;
        }

        let collector = self.collector.read();
        collector
            .generation_manager
            .get_generation(generation)
            .map(|g| g.count)
    }

    pub fn set_threshold(&mut self, generation: usize, threshold: usize) -> GCResult<()> {
        if generation >= 3 {
            return Err(GCError::Internal(format!(
                "Invalid generation: {generation}"
            )));
        }

        self.thresholds[generation] = threshold;
        Ok(())
    }

    pub fn get_threshold(&self, generation: usize) -> Option<usize> {
        self.thresholds.get(generation).copied()
    }

    pub fn collect_if_needed(&self) -> GCResult<usize> {
        if !self.enabled {
            return Ok(0);
        }

        let mut collector = self.collector.write();

        for gen_idx in (0..3).rev() {
            if collector
                .generation_manager
                .get_generation(gen_idx)
                .map(|g| g.should_collect())
                .unwrap_or(false)
            {
                return collector.collect_generation(gen_idx);
            }
        }

        Ok(0)
    }

    pub fn get_uncollectable(&self) -> Vec<PyObject> {
        let collector = self.collector.read();
        collector.uncollectable.clone()
    }

    pub fn clear_uncollectable(&self) {
        let mut collector = self.collector.write();
        collector.uncollectable.clear();
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub mod global {
    use super::*;
    use parking_lot::RwLock;
    use std::sync::Once;

    static INIT: Once = Once::new();
    static mut GC: Option<Arc<RwLock<GarbageCollector>>> = None;

    pub fn get_gc() -> Arc<RwLock<GarbageCollector>> {
        unsafe {
            INIT.call_once(|| {
                GC = Some(Arc::new(RwLock::new(GarbageCollector::new())));
            });

            let gc_ptr = &raw const GC;
            match *gc_ptr {
                Some(ref gc) => gc.clone(),
                None => unreachable!("GC should be initialized by INIT.call_once"),
            }
        }
    }

    pub fn track(obj: PyObject) -> GCResult<()> {
        let binding = get_gc();
        let mut gc = binding.write();
        gc.track(obj)
    }

    pub fn untrack(obj_id: &ObjectId) -> GCResult<()> {
        let binding = get_gc();
        let mut gc = binding.write();
        gc.untrack(obj_id)
    }

    pub fn collect() -> GCResult<usize> {
        let binding = get_gc();
        let gc = binding.read();
        gc.collect()
    }

    pub fn get_stats() -> crate::GCStats {
        let binding = get_gc();
        let gc = binding.read();
        gc.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{ObjectData, PyObject};

    #[test]
    fn test_gc_creation() {
        let gc = GarbageCollector::new();
        assert!(gc.is_enabled());
        assert_eq!(gc.get_count(), 0);
    }

    #[test]
    fn test_object_tracking() {
        let mut gc = GarbageCollector::new();

        let obj = PyObject::new("test".to_string(), ObjectData::Integer(42));
        let obj_id = obj.id;

        assert!(gc.track(obj).is_ok());
        assert_eq!(gc.get_count(), 1);

        assert!(gc.untrack(&obj_id).is_ok());
        assert_eq!(gc.get_count(), 0);
    }

    #[test]
    fn test_generation_thresholds() {
        let mut gc = GarbageCollector::new();

        assert_eq!(gc.get_threshold(0), Some(700));
        assert_eq!(gc.get_threshold(1), Some(10));
        assert_eq!(gc.get_threshold(2), Some(10));

        assert!(gc.set_threshold(0, 1000).is_ok());
        assert_eq!(gc.get_threshold(0), Some(1000));
    }

    #[test]
    fn test_collection() {
        let gc = GarbageCollector::new();

        assert!(gc.collect().is_ok());
        assert_eq!(gc.get_count(), 0);
    }
}
