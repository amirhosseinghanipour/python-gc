#!/usr/bin/env python3
import ctypes
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

lib_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "target/debug/libpython_gc.so")
lib = ctypes.CDLL(lib_path)

GC_SUCCESS = 0
GC_ERROR_INTERNAL = -1
GC_ERROR_NOT_TRACKED = -2
GC_ERROR_ALREADY_TRACKED = -3

def test_finalizer_behavior():
    """Test real-world finalizer behavior"""
    print("Testing Real-World Finalizer Behavior")
    print("=" * 50)
    
    assert lib.py_gc_init() == GC_SUCCESS
    print("✓ GC initialized")
    
    try:
        # Create objects with different finalizer states
        objects = []
        
        # 1. Regular object (no finalizer) - like a list, dict, string
        print("\n1. Creating regular object (no finalizer)...")
        obj1 = {"data": "regular object"}
        obj1_ptr = id(obj1)
        
        # Track it
        result = lib.py_gc_track(obj1_ptr)
        assert result == GC_SUCCESS, f"Failed to track object: {result}"
        print("✓ Object tracked")
        
        # Check finalizer status
        has_finalizer = lib.py_gc_has_finalizer(obj1_ptr)
        print(f"  Finalizer status: {has_finalizer} (0 = no finalizer)")
        assert has_finalizer == 0, "Regular object should not have finalizer"
        
        # 2. Object with finalizer (like a class with __del__)
        print("\n2. Creating object with finalizer...")
        obj2 = {"data": "object with finalizer"}
        obj2_ptr = id(obj2)
        
        # Track it
        result = lib.py_gc_track(obj2_ptr)
        assert result == GC_SUCCESS, f"Failed to track object: {result}"
        print("✓ Object tracked")
        
        # Set finalizer
        result = lib.py_gc_set_finalizer(obj2_ptr, 1)
        assert result == GC_SUCCESS, f"Failed to set finalizer: {result}"
        print("✓ Finalizer set")
        
        # Check finalizer status
        has_finalizer = lib.py_gc_has_finalizer(obj2_ptr)
        print(f"  Finalizer status: {has_finalizer} (1 = has finalizer)")
        assert has_finalizer == 1, "Object should have finalizer after setting it"
        
        # 3. Test object size calculation
        print("\n3. Testing object size calculation...")
        obj3 = {"data": "test object for size"}
        obj3_ptr = id(obj3)
        
        # Track it
        result = lib.py_gc_track(obj3_ptr)
        assert result == GC_SUCCESS, f"Failed to track object: {result}"
        print("✓ Object tracked")
        
        # Get object size
        size = lib.py_gc_get_object_size(obj3_ptr)
        print(f"  Object size: {size} bytes")
        assert size >= 0, "Object size should be non-negative"
        
        # 4. Test finalizer removal
        print("\n4. Testing finalizer removal...")
        obj4 = {"data": "object to remove finalizer"}
        obj4_ptr = id(obj4)
        
        # Track it
        result = lib.py_gc_track(obj4_ptr)
        assert result == GC_SUCCESS, f"Failed to track object: {result}"
        print("✓ Object tracked")
        
        # Set finalizer
        result = lib.py_gc_set_finalizer(obj4_ptr, 1)
        assert result == GC_SUCCESS, f"Failed to set finalizer: {result}"
        print("✓ Finalizer set")
        
        # Remove finalizer
        result = lib.py_gc_set_finalizer(obj4_ptr, 0)
        assert result == GC_SUCCESS, f"Failed to remove finalizer: {result}"
        print("✓ Finalizer removed")
        
        # Check finalizer status
        has_finalizer = lib.py_gc_has_finalizer(obj4_ptr)
        print(f"  Finalizer status: {has_finalizer} (0 = no finalizer)")
        assert has_finalizer == 0, "Object should not have finalizer after removal"
        
        print("\n" + "=" * 50)
        print("✓ All finalizer tests passed!")
        
    finally:
        # Cleanup
        lib.py_gc_cleanup()
        print("✓ GC cleaned up")

if __name__ == "__main__":
    test_finalizer_behavior() 