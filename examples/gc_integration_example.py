#!/usr/bin/env python3
import ctypes
import sys
import os
import time
import gc
import weakref

lib_path = os.path.join(os.path.dirname(__file__), '..', 'target', 'release')
sys.path.insert(0, lib_path)

try:
    lib = ctypes.CDLL(os.path.join(lib_path, 'libpython_gc.so'))
except OSError:
    try:
        lib = ctypes.CDLL(os.path.join(lib_path, 'libpython_gc.dylib'))
    except OSError:
        print("Error: Could not load libpython_gc library")
        sys.exit(1)


class GCStats(ctypes.Structure):
    _fields_ = [
        ("total_tracked", ctypes.c_int32),
        ("generation_counts", ctypes.c_int32 * 3),
        ("uncollectable", ctypes.c_int32)
    ]


GC_SUCCESS = 0
GC_ERROR_ALREADY_TRACKED = -1
GC_ERROR_NOT_TRACKED = -2
GC_ERROR_COLLECTION_IN_PROGRESS = -3
GC_ERROR_INVALID_GENERATION = -4
GC_ERROR_INTERNAL = -5

class PythonGCManager:
    """A Python wrapper around the Rust GC FFI interface"""
    
    def __init__(self):
        self.initialized = False
        self.enabled = False
        self.init()
    
    def init(self):
        """Initialize the garbage collector"""
        if self.initialized:
            return
        
        result = lib.py_gc_init()
        if result == GC_SUCCESS:
            self.initialized = True
            self.enabled = True
            print("✓ GC initialized successfully")
        else:
            raise RuntimeError(f"Failed to initialize GC: {result}")
    
    def cleanup(self):
        """Clean up the garbage collector"""
        if not self.initialized:
            return
        
        result = lib.py_gc_cleanup()
        if result == GC_SUCCESS:
            self.initialized = False
            self.enabled = False
            print("✓ GC cleaned up successfully")
        else:
            print(f"Warning: GC cleanup failed: {result}")
    
    def enable(self):
        """Enable the garbage collector"""
        if not self.initialized:
            raise RuntimeError("GC not initialized")
        
        result = lib.py_gc_enable()
        if result == GC_SUCCESS:
            self.enabled = True
            print("✓ GC enabled")
        else:
            raise RuntimeError(f"Failed to enable GC: {result}")
    
    def disable(self):
        """Disable the garbage collector"""
        if not self.initialized:
            raise RuntimeError("GC not initialized")
        
        result = lib.py_gc_disable()
        if result == GC_SUCCESS:
            self.enabled = False
            print("✓ GC disabled")
        else:
            raise RuntimeError(f"Failed to disable GC: {result}")
    
    def is_enabled(self):
        """Check if the garbage collector is enabled"""
        if not self.initialized:
            return False
        return bool(lib.py_gc_is_enabled())
    
    def get_stats(self):
        """Get garbage collection statistics"""
        if not self.initialized:
            return None
        
        stats = GCStats()
        result = lib.py_gc_get_stats(ctypes.byref(stats))
        if result == GC_SUCCESS:
            return {
                'total_tracked': stats.total_tracked,
                'generation_counts': list(stats.generation_counts),
                'uncollectable': stats.uncollectable
            }
        return None
    
    def get_thresholds(self):
        """Get generation thresholds"""
        if not self.initialized:
            return None
        
        thresholds = []
        for gen in range(3):
            threshold = lib.py_gc_get_threshold(gen)
            thresholds.append(threshold)
        return thresholds
    
    def set_thresholds(self, thresholds):
        """Set generation thresholds"""
        if not self.initialized:
            raise RuntimeError("GC not initialized")
        
        if len(thresholds) != 3:
            raise ValueError("Must provide exactly 3 thresholds")
        
        for gen, threshold in enumerate(thresholds):
            result = lib.py_gc_set_threshold(gen, threshold)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to set threshold for generation {gen}: {result}")
        
        print("✓ Thresholds updated successfully")
    
    def needs_collection(self):
        """Check if collection is needed"""
        if not self.initialized:
            return False
        return bool(lib.py_gc_needs_collection())
    
    def collect(self, generation=None):
        """Perform garbage collection"""
        if not self.initialized:
            raise RuntimeError("GC not initialized")
        
        if generation is not None:
            if not 0 <= generation <= 2:
                raise ValueError("Generation must be 0, 1, or 2")
            result = lib.py_gc_collect_generation(generation)
        else:
            result = lib.py_gc_collect()
        
        if result == GC_SUCCESS:
            return True
        else:
            raise RuntimeError(f"Collection failed: {result}")
    
    def collect_if_needed(self):
        """Collect if thresholds are exceeded"""
        if not self.initialized:
            raise RuntimeError("GC not initialized")
        
        result = lib.py_gc_collect_if_needed()
        if result == GC_SUCCESS:
            return True
        else:
            raise RuntimeError(f"Collection failed: {result}")
    
    def get_state_string(self):
        """Get a string representation of the GC state"""
        if not self.initialized:
            return "GC not initialized"
        
        buffer = ctypes.create_string_buffer(256)
        result = lib.py_gc_get_state_string(buffer, 256)
        if result == GC_SUCCESS:
            return buffer.value.decode('utf-8')
        else:
            return f"Failed to get state: {result}"
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.cleanup()

def create_test_objects(count):
    """Create test objects to demonstrate GC behavior"""
    objects = []
    for i in range(count):
        
        if i % 4 == 0:
            obj = [i] * 10  
        elif i % 4 == 1:
            obj = {f"key_{i}": i}  
        elif i % 4 == 2:
            obj = set(range(i, i + 5))  
        else:
            obj = f"string_{i}" * 5  
        
        objects.append(obj)
    return objects

def demonstrate_gc_behavior():
    """Demonstrate various GC behaviors"""
    print("Advanced Python GC Rust FFI Integration Demo")
    print("=" * 60)
    
    with PythonGCManager() as gc_manager:
        print("\n1. Initial GC State:")
        print(f"   {gc_manager.get_state_string()}")
        
        print("\n2. Current Thresholds:")
        thresholds = gc_manager.get_thresholds()
        for gen, threshold in enumerate(thresholds):
            print(f"   Generation {gen}: {threshold}")
        
        print("\n3. Setting Custom Thresholds:")
        gc_manager.set_thresholds([100, 50, 25])
        
        print("\n4. Creating Test Objects:")
        test_objects = create_test_objects(200)
        print(f"   Created {len(test_objects)} test objects")
        
        print("\n5. Running Python's Built-in GC:")
        collected = gc.collect()
        print(f"   Python GC collected {collected} objects")
        
        print("\n6. Checking Rust GC State:")
        print(f"   {gc_manager.get_state_string()}")
        
        print("\n7. Testing Collection Triggers:")
        needs_collection = gc_manager.needs_collection()
        print(f"   Collection needed: {needs_collection}")
        
        if needs_collection:
            print("   Running collection...")
            gc_manager.collect()
            print(f"   New state: {gc_manager.get_state_string()}")
        
        print("\n8. Testing Generation-Specific Collection:")
        for gen in range(3):
            print(f"   Collecting generation {gen}...")
            start_time = time.time()
            gc_manager.collect(gen)
            end_time = time.time()
            print(f"   Generation {gen} collection completed in {end_time - start_time:.4f}s")
        
        print("\n9. Final Statistics:")
        stats = gc_manager.get_stats()
        if stats:
            print(f"   Total tracked: {stats['total_tracked']}")
            print(f"   Generation 0: {stats['generation_counts'][0]}")
            print(f"   Generation 1: {stats['generation_counts'][1]}")
            print(f"   Generation 2: {stats['generation_counts'][2]}")
            print(f"   Uncollectable: {stats['uncollectable']}")
        
        print("\n10. Performance Test:")
        print("   Creating large number of objects...")
        large_objects = create_test_objects(1000)
        
        start_time = time.time()
        gc_manager.collect()
        end_time = time.time()
        
        print(f"   Full collection completed in {end_time - start_time:.4f}s")
        
        del test_objects
        del large_objects
        
        print("\n11. Final Cleanup:")
        print(f"   Final state: {gc_manager.get_state_string()}")
    
    print("\n" + "=" * 60)
    print("Advanced demo completed successfully!")

def performance_comparison():
    """Compare performance between Python's built-in GC and Rust GC"""
    print("\nPerformance Comparison: Python GC vs Rust GC")
    print("=" * 50)
    
    print("\nTesting Python's built-in GC:")
    start_time = time.time()
    
    objects = create_test_objects(5000)
    collected = gc.collect()
    
    end_time = time.time()
    python_time = end_time - start_time
    
    print(f"   Python GC time: {python_time:.4f}s")
    print(f"   Objects collected: {collected}")
    
    del objects
    
    print("\nTesting Rust GC:")
    with PythonGCManager() as gc_manager:
        start_time = time.time()
        
        objects = create_test_objects(5000)
        gc_manager.collect()
        
        end_time = time.time()
        rust_time = end_time - start_time
        
        print(f"   Rust GC time: {rust_time:.4f}s")
        
        stats = gc_manager.get_stats()
        if stats:
            print(f"   Objects tracked: {stats['total_tracked']}")
    
    del objects
    
    print(f"\nPerformance Summary:")
    print(f"   Python GC: {python_time:.4f}s")
    print(f"   Rust GC: {rust_time:.4f}s")

if __name__ == "__main__":
    try:
        demonstrate_gc_behavior()
        performance_comparison()
    except Exception as e:
        print(f"Error during demo: {e}")
        import traceback
        traceback.print_exc() 