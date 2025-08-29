#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../include/python_gc.h"
#define TEST_ASSERT(condition, message) do { \
    if (condition) { \
        printf("✓ %s\n", message); \
    } else { \
        printf("✗ %s\n", message); \
        exit(1); \
    } \
} while(0)
void test_production_object_tracking() {
    printf("\n=== Testing Production Object Tracking ===\n");
    gc_return_code_t result = py_gc_init();
    TEST_ASSERT(result == GC_SUCCESS, "GC initialization should succeed");
    result = py_gc_clear_registry();
    TEST_ASSERT(result == GC_SUCCESS, "Registry clearing should succeed");
    void* obj1 = malloc(64);
    void* obj2 = malloc(128);
    void* obj3 = malloc(256);
    TEST_ASSERT(obj1 != NULL && obj2 != NULL && obj3 != NULL, 
                "Test object creation should succeed");
    int tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 0, "New objects should not be tracked initially");
    int registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 0, "Registry should be empty initially");
    result = py_gc_track(obj1);
    TEST_ASSERT(result == GC_SUCCESS, "First object tracking should succeed");
    tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 1, "First object should be tracked after tracking");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 1, "Registry should have 1 object");
    result = py_gc_track(obj2);
    TEST_ASSERT(result == GC_SUCCESS, "Second object tracking should succeed");
    result = py_gc_track(obj3);
    TEST_ASSERT(result == GC_SUCCESS, "Third object tracking should succeed");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 3, "Registry should have 3 objects");
    result = py_gc_track(obj1);
    TEST_ASSERT(result == GC_ERROR_ALREADY_TRACKED, "Double tracking should fail");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 3, "Registry count should remain 3 after failed double tracking");
    char buffer[256];
    result = py_gc_get_tracked_info(obj1, buffer, sizeof(buffer));
    TEST_ASSERT(result == GC_SUCCESS, "Getting tracked object info should succeed");
    printf("   Object info: %s\n", buffer);
    result = py_gc_untrack(obj1);
    TEST_ASSERT(result == GC_SUCCESS, "Object untracking should succeed");
    tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 0, "Object should not be tracked after untracking");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 2, "Registry should have 2 objects after untracking");
    result = py_gc_untrack(obj1);
    TEST_ASSERT(result == GC_ERROR_NOT_TRACKED, "Untracking untracked object should fail");
    result = py_gc_clear_registry();
    TEST_ASSERT(result == GC_SUCCESS, "Registry clearing should succeed");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 0, "Registry should be empty after clearing");
    result = py_gc_track(obj1);
    TEST_ASSERT(result == GC_SUCCESS, "Object tracking should succeed after registry clearing");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 1, "Registry should have 1 object after retracking");
    free(obj1);
    free(obj2);
    free(obj3);
    result = py_gc_cleanup();
    TEST_ASSERT(result == GC_SUCCESS, "GC cleanup should succeed");
    printf("✓ Production object tracking tests completed successfully\n");
}
void test_memory_management() {
    printf("\n=== Testing Memory Management ===\n");
    gc_return_code_t result = py_gc_init();
    TEST_ASSERT(result == GC_SUCCESS, "GC initialization should succeed");
    result = py_gc_clear_registry();
    TEST_ASSERT(result == GC_SUCCESS, "Registry clearing should succeed");
    void* objects[20];
    for (int i = 0; i < 20; i++) {
        objects[i] = malloc(64 + i * 8);
        TEST_ASSERT(objects[i] != NULL, "Object creation should succeed");
        printf("   Tracking object %d at %p\n", i, objects[i]);
        result = py_gc_track(objects[i]);
        if (result != GC_SUCCESS) {
            printf("   Failed to track object %d: %d\n", i, result);
        }
        TEST_ASSERT(result == GC_SUCCESS, "Object tracking should succeed");
    }
    int registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 20, "Registry should have 20 objects");
    result = py_gc_collect();
    TEST_ASSERT(result == GC_SUCCESS, "Garbage collection should succeed");
    printf("   GC state before untracking:\n");
    py_gc_debug_state();
    for (int i = 0; i < 10; i++) {
        printf("   Untracking object %d at %p\n", i, objects[i]);
        result = py_gc_debug_untrack(objects[i]);
        if (result != GC_SUCCESS) {
            printf("   Failed to untrack object %d: %d\n", i, result);
        }
        TEST_ASSERT(result == GC_SUCCESS, "Object untracking should succeed");
    }
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 10, "Registry should have 10 objects after partial untracking");
    result = py_gc_clear_registry();
    TEST_ASSERT(result == GC_SUCCESS, "Registry clearing should succeed");
    registry_count = py_gc_get_registry_count();
    TEST_ASSERT(registry_count == 0, "Registry should be empty after clearing");
    for (int i = 0; i < 20; i++) {
        free(objects[i]);
    }
    result = py_gc_cleanup();
    TEST_ASSERT(result == GC_SUCCESS, "GC cleanup should succeed");
    printf("✓ Memory management tests completed successfully\n");
}
void test_error_handling() {
    printf("\n=== Testing Error Handling ===\n");
    gc_return_code_t result = py_gc_init();
    TEST_ASSERT(result == GC_SUCCESS, "GC initialization should succeed");
    result = py_gc_clear_registry();
    TEST_ASSERT(result == GC_SUCCESS, "Registry clearing should succeed");
    result = py_gc_track(NULL);
    TEST_ASSERT(result == GC_ERROR_INTERNAL, "Tracking NULL pointer should fail");
    result = py_gc_untrack(NULL);
    TEST_ASSERT(result == GC_ERROR_INTERNAL, "Untracking NULL pointer should fail");
    int tracked = py_gc_is_tracked(NULL);
    TEST_ASSERT(tracked == 0, "NULL pointer should not be tracked");
    char buffer[256];
    result = py_gc_get_tracked_info(NULL, buffer, sizeof(buffer));
    TEST_ASSERT(result == GC_ERROR_INTERNAL, "Getting info for NULL pointer should fail");
    void* obj = malloc(64);
    TEST_ASSERT(obj != NULL, "Test object creation should succeed");
    result = py_gc_get_tracked_info(obj, buffer, sizeof(buffer));
    TEST_ASSERT(result == GC_ERROR_NOT_TRACKED, "Getting info for untracked object should fail");
    result = py_gc_get_tracked_info(obj, NULL, 256);
    TEST_ASSERT(result == GC_ERROR_INTERNAL, "Getting info with NULL buffer should fail");
    result = py_gc_get_tracked_info(obj, buffer, 0);
    TEST_ASSERT(result == GC_ERROR_INTERNAL, "Getting info with zero buffer size should fail");
    free(obj);
    result = py_gc_cleanup();
    TEST_ASSERT(result == GC_SUCCESS, "GC cleanup should succeed");
    printf("✓ Error handling tests completed successfully\n");
}
int main() {
    printf("Python GC Production Features Test Suite\n");
    printf("========================================\n");
    test_production_object_tracking();
    test_memory_management();
    test_error_handling();
    printf("\n========================================\n");
    printf("✓ All production feature tests passed successfully!\n");
    return 0;
} 