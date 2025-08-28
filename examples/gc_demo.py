#!/usr/bin/env python3
import ctypes
import sys
import os

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

def main():
    print("Python GC Rust FFI Demo")
    print("=" * 40)
    
    print("\n1. Initializing GC...")
    result = lib.py_gc_init()
    if result == GC_SUCCESS:
        print("   ✓ GC initialized successfully")
    else:
        print(f"   ✗ Failed to initialize GC: {result}")
        return
    
    initialized = lib.py_gc_is_initialized()
    print(f"   GC initialized: {bool(initialized)}")
    
    enabled = lib.py_gc_is_enabled()
    print(f"   GC enabled: {bool(enabled)}")
    
    print("\n2. Getting initial statistics...")
    stats = GCStats()
    result = lib.py_gc_get_stats(ctypes.byref(stats))
    if result == GC_SUCCESS:
        print(f"   ✓ Total tracked objects: {stats.total_tracked}")
        print(f"   ✓ Generation 0: {stats.generation_counts[0]}")
        print(f"   ✓ Generation 1: {stats.generation_counts[1]}")
        print(f"   ✓ Generation 2: {stats.generation_counts[2]}")
        print(f"   ✓ Uncollectable: {stats.uncollectable}")
    else:
        print(f"   ✗ Failed to get stats: {result}")
    
    print("\n3. Getting generation thresholds...")
    for gen in range(3):
        threshold = lib.py_gc_get_threshold(gen)
        print(f"   Generation {gen} threshold: {threshold}")
    
    print("\n4. Setting new thresholds...")
    lib.py_gc_set_threshold(0, 1000)
    lib.py_gc_set_threshold(1, 2000)
    lib.py_gc_set_threshold(2, 3000)
    print("   ✓ Thresholds updated")
    
    print("\n5. Checking collection status...")
    needs_collection = lib.py_gc_needs_collection()
    print(f"   Collection needed: {bool(needs_collection)}")
    
    print("\n6. Performing garbage collection...")
    result = lib.py_gc_collect()
    if result == GC_SUCCESS:
        print("   ✓ Collection completed successfully")
    else:
        print(f"   ✗ Collection failed: {result}")
    
    print("\n7. Getting statistics after collection...")
    result = lib.py_gc_get_stats(ctypes.byref(stats))
    if result == GC_SUCCESS:
        print(f"   ✓ Total tracked objects: {stats.total_tracked}")
        print(f"   ✓ Generation 0: {stats.generation_counts[0]}")
        print(f"   ✓ Generation 1: {stats.generation_counts[1]}")
        print(f"   ✓ Generation 2: {stats.generation_counts[2]}")
        print(f"   ✓ Uncollectable: {stats.uncollectable}")
    
    print("\n8. Testing generation-specific collection...")
    for gen in range(3):
        result = lib.py_gc_collect_generation(gen)
        if result == GC_SUCCESS:
            print(f"   ✓ Generation {gen} collection successful")
        else:
            print(f"   ✗ Generation {gen} collection failed: {result}")
    
    print("\n9. Testing error handling...")
    result = lib.py_gc_collect_generation(3)
    if result == GC_ERROR_INVALID_GENERATION:
        print("   ✓ Invalid generation error handled correctly")
    else:
        print(f"   ✗ Expected invalid generation error, got: {result}")
    
    print("\n10. Getting GC state string...")
    buffer = ctypes.create_string_buffer(256)
    result = lib.py_gc_get_state_string(buffer, 256)
    if result == GC_SUCCESS:
        state_str = buffer.value.decode('utf-8')
        print(f"   ✓ GC State: {state_str}")
    else:
        print(f"   ✗ Failed to get state string: {result}")
    
    print("\n11. Cleaning up...")
    result = lib.py_gc_cleanup()
    if result == GC_SUCCESS:
        print("   ✓ GC cleaned up successfully")
    else:
        print(f"   ✗ Cleanup failed: {result}")
    
    initialized = lib.py_gc_is_initialized()
    print(f"   GC initialized after cleanup: {bool(initialized)}")
    
    print("\n" + "=" * 40)
    print("Demo completed successfully!")

if __name__ == "__main__":
    main() 