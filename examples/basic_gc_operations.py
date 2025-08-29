#!/usr/bin/env python3
"""
Simple GC Demo

This demo tests the basic Rust GC functionality without trying to
replace Python's built-in GC, to avoid segmentation faults.
"""

import ctypes
import sys
import os
import time

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

# GC statistics structure
class GCStats(ctypes.Structure):
    _fields_ = [
        ("total_tracked", ctypes.c_int32),
        ("generation_counts", ctypes.c_int32 * 3),
        ("uncollectable", ctypes.c_int32)
    ]

def test_basic_gc_functionality():
    """Test basic GC functionality"""
    print("Testing Basic GC Functionality")
    print("=" * 40)
    
    # Initialize GC
    result = lib.py_gc_init()
    if result != GC_SUCCESS:
        print(f"Failed to initialize GC: {result}")
        return False
    
    print("✓ GC initialized successfully")
    
    # Test enable/disable
    result = lib.py_gc_enable()
    if result != GC_SUCCESS:
        print(f"Failed to enable GC: {result}")
        return False
    
    print("✓ GC enabled successfully")
    
    # Test basic collection
    result = lib.py_gc_collect()
    if result != GC_SUCCESS:
        print(f"Failed to collect: {result}")
        return False
    
    print("✓ Basic collection successful")
    
    # Test generation collection
    for gen in range(3):
        result = lib.py_gc_collect_generation(gen)
        if result != GC_SUCCESS:
            print(f"Failed to collect generation {gen}: {result}")
            return False
    
    print("✓ Generation collection successful")
    
    # Test statistics
    stats = GCStats()
    result = lib.py_gc_get_stats(ctypes.byref(stats))
    if result != GC_SUCCESS:
        print(f"Failed to get stats: {result}")
        return False
    
    print(f"✓ Statistics retrieved: tracked={stats.total_tracked}, "
          f"gen0={stats.generation_counts[0]}, "
          f"gen1={stats.generation_counts[1]}, "
          f"gen2={stats.generation_counts[2]}, "
          f"uncollectable={stats.uncollectable}")
    
    # Test threshold management
    for gen in range(3):
        result = lib.py_gc_set_threshold(gen, 100 + gen * 50)
        if result != GC_SUCCESS:
            print(f"Failed to set threshold for generation {gen}: {result}")
            return False
        
        threshold = lib.py_gc_get_threshold(gen)
        if threshold != 100 + gen * 50:
            print(f"Threshold mismatch for generation {gen}: expected {100 + gen * 50}, got {threshold}")
            return False
    
    print("✓ Threshold management successful")
    
    # Test state string
    buffer = ctypes.create_string_buffer(256)
    result = lib.py_gc_get_state_string(buffer, 256)
    if result != GC_SUCCESS:
        print(f"Failed to get state string: {result}")
        return False
    
    state_str = buffer.value.decode('utf-8')
    print(f"✓ State string: {state_str}")
    
    # Test needs collection
    needs_collection = lib.py_gc_needs_collection()
    print(f"✓ Needs collection: {needs_collection}")
    
    # Test collect if needed
    result = lib.py_gc_collect_if_needed()
    if result != GC_SUCCESS:
        print(f"Failed to collect if needed: {result}")
        return False
    
    print("✓ Collect if needed successful")
    
    # Test debug state
    result = lib.py_gc_debug_state()
    if result != GC_SUCCESS:
        print(f"Failed to get debug state: {result}")
        return False
    
    print("✓ Debug state successful")
    
    # Cleanup
    result = lib.py_gc_cleanup()
    if result != GC_SUCCESS:
        print(f"Failed to cleanup GC: {result}")
        return False
    
    print("✓ GC cleanup successful")
    
    return True

def test_object_tracking():
    """Test object tracking functionality"""
    print("\nTesting Object Tracking")
    print("=" * 40)
    
    # Initialize GC
    result = lib.py_gc_init()
    if result != GC_SUCCESS:
        print(f"Failed to initialize GC: {result}")
        return False
    
    # Create some test objects (using their memory addresses)
    test_objects = []
    for i in range(10):
        # Create a simple object and get its address
        obj = [i] * 10
        obj_ptr = id(obj)
        test_objects.append((obj, obj_ptr))
    
    print(f"Created {len(test_objects)} test objects")
    
    # Track objects
    for obj, obj_ptr in test_objects:
        result = lib.py_gc_track(obj_ptr)
        if result != GC_SUCCESS:
            print(f"Failed to track object {obj_ptr}: {result}")
            return False
    
    print("✓ All objects tracked successfully")
    
    # Check if objects are tracked
    for obj, obj_ptr in test_objects:
        is_tracked = lib.py_gc_is_tracked(obj_ptr)
        if not is_tracked:
            print(f"Object {obj_ptr} is not tracked")
            return False
    
    print("✓ All objects confirmed as tracked")
    
    # Get object info
    for obj, obj_ptr in test_objects[:3]:  # Test first 3 objects
        buffer = ctypes.create_string_buffer(256)
        result = lib.py_gc_get_tracked_info(obj_ptr, buffer, 256)
        if result == GC_SUCCESS:
            info = buffer.value.decode('utf-8')
            print(f"✓ Object {obj_ptr} info: {info}")
        else:
            print(f"Failed to get info for object {obj_ptr}: {result}")
    
    # Get object size
    for obj, obj_ptr in test_objects[:3]:
        size = lib.py_gc_get_object_size(obj_ptr)
        print(f"✓ Object {obj_ptr} size: {size} bytes")
    
    # Get object type
    for obj, obj_ptr in test_objects[:3]:
        buffer = ctypes.create_string_buffer(64)
        result = lib.py_gc_get_object_type_name(obj_ptr, buffer, 64)
        if result == GC_SUCCESS:
            obj_type = buffer.value.decode('utf-8')
            print(f"✓ Object {obj_ptr} type: {obj_type}")
    
    # Test finalizer management
    for obj, obj_ptr in test_objects[:3]:
        # Set finalizer
        result = lib.py_gc_set_finalizer(obj_ptr, 1)
        if result != GC_SUCCESS:
            print(f"Failed to set finalizer for object {obj_ptr}: {result}")
            return False
        
        # Check finalizer
        has_finalizer = lib.py_gc_has_finalizer(obj_ptr)
        if not has_finalizer:
            print(f"Finalizer not set for object {obj_ptr}")
            return False
        
        # Clear finalizer
        result = lib.py_gc_set_finalizer(obj_ptr, 0)
        if result != GC_SUCCESS:
            print(f"Failed to clear finalizer for object {obj_ptr}: {result}")
            return False
    
    print("✓ Finalizer management successful")
    
    # Test reference counting
    for obj, obj_ptr in test_objects[:3]:
        # Set reference count
        result = lib.py_gc_set_refcount(obj_ptr, 5)
        if result != GC_SUCCESS:
            print(f"Failed to set refcount for object {obj_ptr}: {result}")
            return False
        
        # Get reference count
        refcount = lib.py_gc_get_refcount(obj_ptr)
        if refcount != 5:
            print(f"Refcount mismatch for object {obj_ptr}: expected 5, got {refcount}")
            return False
    
    print("✓ Reference counting successful")
    
    # Perform collection
    result = lib.py_gc_collect()
    if result != GC_SUCCESS:
        print(f"Failed to collect: {result}")
        return False
    
    print("✓ Collection successful")
    
    # Untrack objects
    for obj, obj_ptr in test_objects:
        result = lib.py_gc_untrack(obj_ptr)
        if result != GC_SUCCESS:
            print(f"Failed to untrack object {obj_ptr}: {result}")
            return False
    
    print("✓ All objects untracked successfully")
    
    # Verify objects are untracked
    for obj, obj_ptr in test_objects:
        is_tracked = lib.py_gc_is_tracked(obj_ptr)
        if is_tracked:
            print(f"Object {obj_ptr} is still tracked")
            return False
    
    print("✓ All objects confirmed as untracked")
    
    # Cleanup
    result = lib.py_gc_cleanup()
    if result != GC_SUCCESS:
        print(f"Failed to cleanup GC: {result}")
        return False
    
    print("✓ GC cleanup successful")
    
    return True

def test_performance():
    """Test GC performance"""
    print("\nTesting Performance")
    print("=" * 40)
    
    # Initialize GC
    result = lib.py_gc_init()
    if result != GC_SUCCESS:
        print(f"Failed to initialize GC: {result}")
        return False
    
    # Create many objects
    num_objects = 10000
    objects = []
    
    print(f"Creating {num_objects} objects...")
    start_time = time.time()
    
    for i in range(num_objects):
        obj = [i] * 10
        obj_ptr = id(obj)
        objects.append((obj, obj_ptr))
    
    creation_time = time.time() - start_time
    print(f"✓ Object creation: {creation_time:.4f}s")
    
    # Track objects
    print("Tracking objects...")
    start_time = time.time()
    
    for obj, obj_ptr in objects:
        result = lib.py_gc_track(obj_ptr)
        if result != GC_SUCCESS:
            print(f"Failed to track object {obj_ptr}: {result}")
            return False
    
    tracking_time = time.time() - start_time
    print(f"✓ Object tracking: {tracking_time:.4f}s")
    
    # Perform collection
    print("Performing collection...")
    start_time = time.time()
    
    result = lib.py_gc_collect()
    if result != GC_SUCCESS:
        print(f"Failed to collect: {result}")
        return False
    
    collection_time = time.time() - start_time
    print(f"✓ Collection: {collection_time:.4f}s")
    
    # Get statistics
    stats = GCStats()
    result = lib.py_gc_get_stats(ctypes.byref(stats))
    if result == GC_SUCCESS:
        print(f"✓ Final stats: tracked={stats.total_tracked}, "
              f"gen0={stats.generation_counts[0]}, "
              f"gen1={stats.generation_counts[1]}, "
              f"gen2={stats.generation_counts[2]}")
    
    # Untrack objects
    print("Untracking objects...")
    start_time = time.time()
    
    for obj, obj_ptr in objects:
        result = lib.py_gc_untrack(obj_ptr)
        if result != GC_SUCCESS:
            print(f"Failed to untrack object {obj_ptr}: {result}")
            return False
    
    untracking_time = time.time() - start_time
    print(f"✓ Object untracking: {untracking_time:.4f}s")
    
    # Performance summary
    total_time = creation_time + tracking_time + collection_time + untracking_time
    print(f"\nPerformance Summary:")
    print(f"  Creation: {creation_time:.4f}s")
    print(f"  Tracking: {tracking_time:.4f}s")
    print(f"  Collection: {collection_time:.4f}s")
    print(f"  Untracking: {untracking_time:.4f}s")
    print(f"  Total: {total_time:.4f}s")
    print(f"  Objects per second: {num_objects / total_time:.0f}")
    
    # Cleanup
    result = lib.py_gc_cleanup()
    if result != GC_SUCCESS:
        print(f"Failed to cleanup GC: {result}")
        return False
    
    print("✓ GC cleanup successful")
    
    return True

def main():
    """Main test function"""
    print("Rust GC Simple Demo")
    print("=" * 60)
    
    tests = [
        ("Basic GC Functionality", test_basic_gc_functionality),
        ("Object Tracking", test_object_tracking),
        ("Performance", test_performance),
    ]
    
    passed = 0
    total = len(tests)
    
    for test_name, test_func in tests:
        try:
            if test_func():
                passed += 1
                print(f"✓ {test_name} PASSED")
            else:
                print(f"✗ {test_name} FAILED")
        except Exception as e:
            print(f"✗ {test_name} FAILED with exception: {e}")
            import traceback
            traceback.print_exc()
    
    print("\n" + "=" * 60)
    print(f"Test Results: {passed}/{total} tests passed")
    
    if passed == total:
        print("🎉 All tests passed! The Rust GC is working correctly.")
    else:
        print("❌ Some tests failed. Please check the implementation.")
    
    return passed == total

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1) 