#!/usr/bin/env python3
"""
Python GC Replacement Module

This module provides a complete replacement for Python's built-in gc module,
using the Rust garbage collector implementation.

Usage:
    import python_gc_replacement as gc
    # Use exactly like Python's built-in gc module
"""

import ctypes
import sys
import os
import weakref
import threading
from typing import List, Optional, Tuple, Any, Dict, Set
import atexit

# Load the Rust GC library
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

# Return codes
GC_SUCCESS = 0
GC_ERROR_ALREADY_TRACKED = -1
GC_ERROR_NOT_TRACKED = -2
GC_ERROR_COLLECTION_IN_PROGRESS = -3
GC_ERROR_INVALID_GENERATION = -4
GC_ERROR_INTERNAL = -5

# Debug flags (matching Python's gc module)
DEBUG_STATS = 1
DEBUG_COLLECTABLE = 2
DEBUG_UNCOLLECTABLE = 4
DEBUG_INSTANCES = 8
DEBUG_OBJECTS = 16
DEBUG_SAVEALL = 32
DEBUG_LEAK = 64

# GC statistics structure
class GCStats(ctypes.Structure):
    _fields_ = [
        ("total_tracked", ctypes.c_int32),
        ("generation_counts", ctypes.c_int32 * 3),
        ("uncollectable", ctypes.c_int32)
    ]

class PythonGCReplacement:
    """
    Complete replacement for Python's built-in gc module.
    
    This class provides all the functionality of Python's gc module
    but uses the Rust garbage collector implementation instead.
    """
    
    def __init__(self):
        self._initialized = False
        self._enabled = False
        self._automatic_tracking = False
        self._debug_flags = 0
        self._garbage = []
        self._callbacks = []
        self._lock = threading.RLock()
        
        # Initialize the GC
        self._init_gc()
        
        # Register cleanup on exit
        atexit.register(self._cleanup)
    
    def _init_gc(self):
        """Initialize the garbage collector"""
        with self._lock:
            if self._initialized:
                return
            
            result = lib.py_gc_init()
            if result == GC_SUCCESS:
                self._initialized = True
                self._enabled = True
                print("✓ Rust GC initialized successfully")
            else:
                raise RuntimeError(f"Failed to initialize Rust GC: {result}")
    
    def _cleanup(self):
        """Clean up the garbage collector"""
        with self._lock:
            if not self._initialized:
                return
            
            result = lib.py_gc_cleanup()
            if result == GC_SUCCESS:
                self._initialized = False
                self._enabled = False
                print("✓ Rust GC cleaned up successfully")
            else:
                print(f"Warning: Rust GC cleanup failed: {result}")
    
    def enable(self):
        """Enable automatic garbage collection"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_enable()
            if result == GC_SUCCESS:
                self._enabled = True
            else:
                raise RuntimeError(f"Failed to enable GC: {result}")
    
    def disable(self):
        """Disable automatic garbage collection"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_disable()
            if result == GC_SUCCESS:
                self._enabled = False
            else:
                raise RuntimeError(f"Failed to disable GC: {result}")
    
    def isenabled(self):
        """Check if automatic collection is enabled"""
        with self._lock:
            if not self._initialized:
                return False
            return bool(lib.py_gc_is_enabled())
    
    def enable_automatic_tracking(self):
        """Enable automatic tracking of Python objects"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_enable_automatic_tracking()
            if result == GC_SUCCESS:
                self._automatic_tracking = True
            else:
                raise RuntimeError(f"Failed to enable automatic tracking: {result}")
    
    def disable_automatic_tracking(self):
        """Disable automatic tracking of Python objects"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_disable_automatic_tracking()
            if result == GC_SUCCESS:
                self._automatic_tracking = False
            else:
                raise RuntimeError(f"Failed to disable automatic tracking: {result}")
    
    def is_automatic_tracking_enabled(self):
        """Check if automatic tracking is enabled"""
        with self._lock:
            if not self._initialized:
                return False
            return bool(lib.py_gc_is_automatic_tracking_enabled())
    
    def collect(self, generation=None):
        """
        Perform garbage collection.
        
        Args:
            generation: Generation to collect (0, 1, 2, or None for all)
        
        Returns:
            Number of objects collected
        """
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            if generation is not None:
                if not 0 <= generation <= 2:
                    raise ValueError("Generation must be 0, 1, or 2")
                result = lib.py_gc_collect_generation(generation)
            else:
                result = lib.py_gc_collect()
            
            if result == GC_SUCCESS:
                # Get the number of objects collected
                stats = self.get_stats()
                return stats['total_tracked'] if stats else 0
            else:
                raise RuntimeError(f"Collection failed: {result}")
    
    def collect_if_needed(self):
        """Collect if thresholds are exceeded"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_collect_if_needed()
            if result == GC_SUCCESS:
                return True
            else:
                raise RuntimeError(f"Collection failed: {result}")
    
    def get_count(self):
        """
        Get collection counts for all generations.
        
        Returns:
            Tuple of (count0, count1, count2)
        """
        with self._lock:
            if not self._initialized:
                return (0, 0, 0)
            
            counts_ptr = lib.py_gc_get_collection_counts()
            if counts_ptr:
                try:
                    counts = ctypes.cast(counts_ptr, ctypes.POINTER(ctypes.c_int * 3)).contents
                    return tuple(counts)
                finally:
                    lib.py_gc_free_collection_counts(counts_ptr)
            else:
                return (0, 0, 0)
    
    def get_stats(self):
        """Get garbage collection statistics"""
        with self._lock:
            if not self._initialized:
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
    
    def set_threshold(self, generation, threshold):
        """Set threshold for a generation"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            if not 0 <= generation <= 2:
                raise ValueError("Generation must be 0, 1, or 2")
            
            result = lib.py_gc_set_threshold(generation, threshold)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to set threshold: {result}")
    
    def get_threshold(self, generation):
        """Get threshold for a generation"""
        with self._lock:
            if not self._initialized:
                return 0
            
            if not 0 <= generation <= 2:
                raise ValueError("Generation must be 0, 1, or 2")
            
            threshold = lib.py_gc_get_threshold(generation)
            return threshold if threshold >= 0 else 0
    
    def set_debug(self, flags):
        """Set debug flags"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            result = lib.py_gc_set_debug_flags(flags)
            if result == GC_SUCCESS:
                self._debug_flags = flags
            else:
                raise RuntimeError(f"Failed to set debug flags: {result}")
    
    def get_debug(self):
        """Get current debug flags"""
        with self._lock:
            if not self._initialized:
                return 0
            return lib.py_gc_get_debug_flags()
    
    def is_tracked(self, obj):
        """Check if an object is tracked by the garbage collector"""
        with self._lock:
            if not self._initialized:
                return False
            
            obj_ptr = id(obj)
            return bool(lib.py_gc_is_tracked_python(obj_ptr))
    
    def track(self, obj):
        """Track an object for garbage collection"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            obj_ptr = id(obj)
            result = lib.py_gc_track_python(obj_ptr)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to track object: {result}")
    
    def untrack(self, obj):
        """Stop tracking an object"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            obj_ptr = id(obj)
            result = lib.py_gc_untrack_python(obj_ptr)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to untrack object: {result}")
    
    def get_objects(self):
        """
        Get all tracked objects.
        
        Note: This is a placeholder implementation.
        In a full implementation, this would return actual Python objects.
        """
        with self._lock:
            if not self._initialized:
                return []
            
            # For now, return an empty list
            # In a full implementation, this would query the Rust GC
            # and convert the tracked objects back to Python objects
            return []
    
    def get_referrers(self, obj):
        """
        Get objects that refer to the given object.
        
        Note: This is a placeholder implementation.
        """
        with self._lock:
            if not self._initialized:
                return []
            
            # For now, return an empty list
            # In a full implementation, this would query the Rust GC
            # for objects that reference the given object
            return []
    
    def get_referents(self, obj):
        """
        Get objects that the given object refers to.
        
        Note: This is a placeholder implementation.
        """
        with self._lock:
            if not self._initialized:
                return []
            
            # For now, return an empty list
            # In a full implementation, this would query the Rust GC
            # for objects referenced by the given object
            return []
    
    def get_garbage(self):
        """Get uncollectable objects"""
        with self._lock:
            return self._garbage.copy()
    
    def set_garbage(self, garbage_list):
        """Set the list of uncollectable objects"""
        with self._lock:
            self._garbage = list(garbage_list) if garbage_list else []
    
    def clear_garbage(self):
        """Clear the list of uncollectable objects"""
        with self._lock:
            self._garbage.clear()
    
    def add_callback(self, callback):
        """Add a callback to be called before collection"""
        with self._lock:
            if callback not in self._callbacks:
                self._callbacks.append(callback)
    
    def remove_callback(self, callback):
        """Remove a callback"""
        with self._lock:
            if callback in self._callbacks:
                self._callbacks.remove(callback)
    
    def callbacks(self):
        """Get all registered callbacks"""
        with self._lock:
            return self._callbacks.copy()
    
    def get_object_info(self, obj):
        """Get detailed information about a tracked object"""
        with self._lock:
            if not self._initialized:
                return None
            
            obj_ptr = id(obj)
            buffer = ctypes.create_string_buffer(256)
            result = lib.py_gc_get_tracked_info(obj_ptr, buffer, 256)
            
            if result == GC_SUCCESS:
                return buffer.value.decode('utf-8')
            else:
                return None
    
    def get_object_size(self, obj):
        """Get the size of a tracked object in bytes"""
        with self._lock:
            if not self._initialized:
                return 0
            
            obj_ptr = id(obj)
            return lib.py_gc_get_object_size(obj_ptr)
    
    def get_object_type(self, obj):
        """Get the type name of a tracked object"""
        with self._lock:
            if not self._initialized:
                return "unknown"
            
            obj_ptr = id(obj)
            buffer = ctypes.create_string_buffer(64)
            result = lib.py_gc_get_object_type_name(obj_ptr, buffer, 64)
            
            if result == GC_SUCCESS:
                return buffer.value.decode('utf-8')
            else:
                return "unknown"
    
    def has_finalizer(self, obj):
        """Check if an object has a finalizer"""
        with self._lock:
            if not self._initialized:
                return False
            
            obj_ptr = id(obj)
            return bool(lib.py_gc_has_finalizer(obj_ptr))
    
    def set_finalizer(self, obj, has_finalizer):
        """Set whether an object has a finalizer"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            obj_ptr = id(obj)
            result = lib.py_gc_set_finalizer(obj_ptr, 1 if has_finalizer else 0)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to set finalizer: {result}")
    
    def get_refcount(self, obj):
        """Get the reference count of an object"""
        with self._lock:
            if not self._initialized:
                return 0
            
            obj_ptr = id(obj)
            return lib.py_gc_get_refcount(obj_ptr)
    
    def set_refcount(self, obj, refcount):
        """Set the reference count of an object"""
        with self._lock:
            if not self._initialized:
                raise RuntimeError("GC not initialized")
            
            obj_ptr = id(obj)
            result = lib.py_gc_set_refcount(obj_ptr, refcount)
            if result != GC_SUCCESS:
                raise RuntimeError(f"Failed to set reference count: {result}")
    
    def needs_collection(self):
        """Check if collection is needed"""
        with self._lock:
            if not self._initialized:
                return False
            return bool(lib.py_gc_needs_collection())
    
    def get_state_string(self):
        """Get a string representation of the GC state"""
        with self._lock:
            if not self._initialized:
                return "GC not initialized"
            
            buffer = ctypes.create_string_buffer(256)
            result = lib.py_gc_get_state_string(buffer, 256)
            
            if result == GC_SUCCESS:
                return buffer.value.decode('utf-8')
            else:
                return f"Failed to get state: {result}"
    
    def debug_state(self):
        """Print debug state information"""
        with self._lock:
            if not self._initialized:
                print("GC not initialized")
                return
            
            result = lib.py_gc_debug_state()
            if result != GC_SUCCESS:
                print(f"Failed to get debug state: {result}")

# Create a global instance
_gc_instance = PythonGCReplacement()

# Export all the methods as module-level functions
def enable():
    """Enable automatic garbage collection"""
    return _gc_instance.enable()

def disable():
    """Disable automatic garbage collection"""
    return _gc_instance.disable()

def isenabled():
    """Check if automatic collection is enabled"""
    return _gc_instance.isenabled()

def collect(generation=None):
    """Perform garbage collection"""
    return _gc_instance.collect(generation)

def get_count():
    """Get collection counts for all generations"""
    return _gc_instance.get_count()

def get_stats():
    """Get garbage collection statistics"""
    return _gc_instance.get_stats()

def set_threshold(generation, threshold):
    """Set threshold for a generation"""
    return _gc_instance.set_threshold(generation, threshold)

def get_threshold(generation):
    """Get threshold for a generation"""
    return _gc_instance.get_threshold(generation)

def set_debug(flags):
    """Set debug flags"""
    return _gc_instance.set_debug(flags)

def get_debug():
    """Get current debug flags"""
    return _gc_instance.get_debug()

def is_tracked(obj):
    """Check if an object is tracked by the garbage collector"""
    return _gc_instance.is_tracked(obj)

def track(obj):
    """Track an object for garbage collection"""
    return _gc_instance.track(obj)

def untrack(obj):
    """Stop tracking an object"""
    return _gc_instance.untrack(obj)

def get_objects():
    """Get all tracked objects"""
    return _gc_instance.get_objects()

def get_referrers(obj):
    """Get objects that refer to the given object"""
    return _gc_instance.get_referrers(obj)

def get_referents(obj):
    """Get objects that the given object refers to"""
    return _gc_instance.get_referents(obj)

def get_garbage():
    """Get uncollectable objects"""
    return _gc_instance.get_garbage()

def set_garbage(garbage_list):
    """Set the list of uncollectable objects"""
    return _gc_instance.set_garbage(garbage_list)

def add_callback(callback):
    """Add a callback to be called before collection"""
    return _gc_instance.add_callback(callback)

def remove_callback(callback):
    """Remove a callback"""
    return _gc_instance.remove_callback(callback)

def callbacks():
    """Get all registered callbacks"""
    return _gc_instance.callbacks()

# Additional utility functions
def enable_automatic_tracking():
    """Enable automatic tracking of Python objects"""
    return _gc_instance.enable_automatic_tracking()

def disable_automatic_tracking():
    """Disable automatic tracking of Python objects"""
    return _gc_instance.disable_automatic_tracking()

def is_automatic_tracking_enabled():
    """Check if automatic tracking is enabled"""
    return _gc_instance.is_automatic_tracking_enabled()

def collect_if_needed():
    """Collect if thresholds are exceeded"""
    return _gc_instance.collect_if_needed()

def needs_collection():
    """Check if collection is needed"""
    return _gc_instance.needs_collection()

def get_object_info(obj):
    """Get detailed information about a tracked object"""
    return _gc_instance.get_object_info(obj)

def get_object_size(obj):
    """Get the size of a tracked object in bytes"""
    return _gc_instance.get_object_size(obj)

def get_object_type(obj):
    """Get the type name of a tracked object"""
    return _gc_instance.get_object_type(obj)

def has_finalizer(obj):
    """Check if an object has a finalizer"""
    return _gc_instance.has_finalizer(obj)

def set_finalizer(obj, has_finalizer):
    """Set whether an object has a finalizer"""
    return _gc_instance.set_finalizer(obj, has_finalizer)

def get_refcount(obj):
    """Get the reference count of an object"""
    return _gc_instance.get_refcount(obj)

def set_refcount(obj, refcount):
    """Set the reference count of an object"""
    return _gc_instance.set_refcount(obj, refcount)

def get_state_string():
    """Get a string representation of the GC state"""
    return _gc_instance.get_state_string()

def debug_state():
    """Print debug state information"""
    return _gc_instance.debug_state()

# Export constants
__all__ = [
    'enable', 'disable', 'isenabled', 'collect', 'get_count', 'get_stats',
    'set_threshold', 'get_threshold', 'set_debug', 'get_debug',
    'is_tracked', 'track', 'untrack', 'get_objects', 'get_referrers',
    'get_referents', 'get_garbage', 'set_garbage', 'add_callback',
    'remove_callback', 'callbacks', 'enable_automatic_tracking',
    'disable_automatic_tracking', 'is_automatic_tracking_enabled',
    'collect_if_needed', 'needs_collection', 'get_object_info',
    'get_object_size', 'get_object_type', 'has_finalizer', 'set_finalizer',
    'get_refcount', 'set_refcount', 'get_state_string', 'debug_state',
    'DEBUG_STATS', 'DEBUG_COLLECTABLE', 'DEBUG_UNCOLLECTABLE',
    'DEBUG_INSTANCES', 'DEBUG_OBJECTS', 'DEBUG_SAVEALL', 'DEBUG_LEAK'
]

if __name__ == "__main__":
    # Demo the GC replacement
    print("Python GC Replacement Demo")
    print("=" * 40)
    
    # Initialize and enable automatic tracking
    enable_automatic_tracking()
    print(f"Automatic tracking enabled: {is_automatic_tracking_enabled()}")
    
    # Create some test objects
    test_list = [1, 2, 3]
    test_dict = {"a": 1, "b": 2}
    test_set = {1, 2, 3}
    
    # Track them manually
    track(test_list)
    track(test_dict)
    track(test_set)
    
    print(f"Objects tracked: {get_count()}")
    print(f"GC state: {get_state_string()}")
    
    # Perform collection
    collected = collect()
    print(f"Objects collected: {collected}")
    
    # Get statistics
    stats = get_stats()
    if stats:
        print(f"Statistics: {stats}")
    
    # Clean up
    untrack(test_list)
    untrack(test_dict)
    untrack(test_set)
    
    print("Demo completed successfully!") 