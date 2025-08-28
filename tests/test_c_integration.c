#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "../include/python_gc.h"

typedef struct {
    int total_tests;
    int passed_tests;
    int failed_tests;
} test_results_t;

static test_results_t test_results = {0, 0, 0};

#define TEST_ASSERT(condition, message) do { \
    test_results.total_tests++; \
    if (condition) { \
        test_results.passed_tests++; \
        printf("✓ %s\n", message); \
    } else { \
        test_results.failed_tests++; \
        printf("✗ %s\n", message); \
    } \
} while(0)

static void test_gc_initialization(void);
static void test_gc_enable_disable(void);
static void test_gc_object_tracking(void);
static void test_gc_collection(void);
static void test_gc_statistics(void);
static void test_gc_thresholds(void);
static void test_gc_error_handling(void);
static void test_gc_cleanup(void);

static void* create_mock_object(size_t size) {
    void* obj = malloc(size);
    if (obj) {
        memset(obj, 0xAA, size);
    }
    return obj;
}

static void destroy_mock_object(void* obj) {
    if (obj) {
        free(obj);
    }
}

static void test_gc_initialization(void) {
    printf("\n=== Testing GC Initialization ===\n");
    
    gc_return_code_t result = py_gc_init();
    TEST_ASSERT(result == GC_SUCCESS, "GC initialization should succeed");
    
    result = py_gc_init();
    TEST_ASSERT(result == GC_SUCCESS, "Double initialization should succeed");
}

static void test_gc_enable_disable(void) {
    printf("\n=== Testing GC Enable/Disable ===\n");
    
    int enabled = py_gc_is_enabled();
    TEST_ASSERT(enabled == 1, "GC should be enabled by default after initialization");
    
    gc_return_code_t result = py_gc_disable();
    TEST_ASSERT(result == GC_SUCCESS, "GC disable should succeed");
    
    enabled = py_gc_is_enabled();
    TEST_ASSERT(enabled == 0, "GC should be disabled after disable call");
    
    result = py_gc_enable();
    TEST_ASSERT(result == GC_SUCCESS, "GC enable should succeed");
    
    enabled = py_gc_is_enabled();
    TEST_ASSERT(enabled == 1, "GC should be enabled after enable call");
}

static void test_gc_object_tracking(void) {
    printf("\n=== Testing Object Tracking ===\n");
    
    void* obj1 = create_mock_object(64);
    void* obj2 = create_mock_object(128);
    void* obj3 = create_mock_object(256);
    
    TEST_ASSERT(obj1 != NULL && obj2 != NULL && obj3 != NULL, 
                "Mock object creation should succeed");
    
    int tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 0, "New objects should not be tracked initially");
    
    gc_return_code_t result = py_gc_track(obj1);
    TEST_ASSERT(result == GC_SUCCESS, "Object tracking should succeed");
    
    tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 1, "Object should be tracked after tracking");
    
    result = py_gc_track(obj1);
    TEST_ASSERT(result == GC_ERROR_ALREADY_TRACKED, "Double tracking should fail with ALREADY_TRACKED");
    
    result = py_gc_track(obj2);
    TEST_ASSERT(result == GC_SUCCESS, "Second object tracking should succeed");
    
    result = py_gc_track(obj3);
    TEST_ASSERT(result == GC_SUCCESS, "Third object tracking should succeed");
    
    result = py_gc_untrack(obj1);
    TEST_ASSERT(result == GC_SUCCESS, "Object untracking should succeed");
    
    tracked = py_gc_is_tracked(obj1);
    TEST_ASSERT(tracked == 0, "Object should not be tracked after untracking");
    
    result = py_gc_untrack(obj1);
    TEST_ASSERT(result == GC_ERROR_NOT_TRACKED, "Untracking untracked object should fail with NOT_TRACKED");
    
    destroy_mock_object(obj1);
    destroy_mock_object(obj2);
    destroy_mock_object(obj3);
}

static void test_gc_collection(void) {
    printf("\n=== Testing Garbage Collection ===\n");
    
    int needs_collection = py_gc_needs_collection();
    TEST_ASSERT(needs_collection == 0 || needs_collection == 1, 
                "Collection need check should return valid boolean");
    
    gc_return_code_t result = py_gc_collect_generation(0);
    TEST_ASSERT(result == GC_SUCCESS, "Generation 0 collection should succeed");
    
    result = py_gc_collect_generation(1);
    TEST_ASSERT(result == GC_SUCCESS, "Generation 1 collection should succeed");
    
    result = py_gc_collect_generation(2);
    TEST_ASSERT(result == GC_SUCCESS, "Generation 2 collection should succeed");
    
    result = py_gc_collect_generation(3);
    TEST_ASSERT(result == GC_ERROR_INVALID_GENERATION, 
                "Invalid generation should return appropriate error");
    
    result = py_gc_collect_generation(-1);
    TEST_ASSERT(result == GC_ERROR_INVALID_GENERATION, 
                "Negative generation should return appropriate error");
    
    result = py_gc_collect();
    TEST_ASSERT(result == GC_SUCCESS, "Full collection should succeed");
    
    result = py_gc_collect_if_needed();
    TEST_ASSERT(result == GC_SUCCESS, "Conditional collection should succeed");
}

static void test_gc_statistics(void) {
    printf("\n=== Testing GC Statistics ===\n");
    
    gc_stats_t stats;
    gc_return_code_t result = py_gc_get_stats(&stats);
    TEST_ASSERT(result == GC_SUCCESS, "Statistics retrieval should succeed");
    
    TEST_ASSERT(stats.total_tracked >= 0, "Total tracked count should be non-negative");
    TEST_ASSERT(stats.generation_counts[0] >= 0, "Generation 0 count should be non-negative");
    TEST_ASSERT(stats.generation_counts[1] >= 0, "Generation 1 count should be non-negative");
    TEST_ASSERT(stats.generation_counts[2] >= 0, "Generation 2 count should be non-negative");
    TEST_ASSERT(stats.uncollectable >= 0, "Uncollectable count should be non-negative");
    
    int32_t total_count = py_gc_get_count();
    TEST_ASSERT(total_count >= 0, "Total count should be non-negative");
    TEST_ASSERT(total_count == stats.total_tracked, 
                "Individual count should match statistics total");
    
    int32_t gen0_count = py_gc_get_generation_count(0);
    TEST_ASSERT(gen0_count >= 0, "Generation 0 count should be non-negative");
    TEST_ASSERT(gen0_count == stats.generation_counts[0], 
                "Individual generation count should match statistics");
    
    int32_t uncollectable_count = py_gc_get_uncollectable_count();
    TEST_ASSERT(uncollectable_count >= 0, "Uncollectable count should be non-negative");
    TEST_ASSERT(uncollectable_count == stats.uncollectable, 
                "Individual uncollectable count should match statistics");
}

static void test_gc_thresholds(void) {
    printf("\n=== Testing GC Thresholds ===\n");
    
    int32_t threshold0 = py_gc_get_threshold(0);
    int32_t threshold1 = py_gc_get_threshold(1);
    int32_t threshold2 = py_gc_get_threshold(2);
    
    TEST_ASSERT(threshold0 >= 0, "Generation 0 threshold should be valid");
    TEST_ASSERT(threshold1 >= 0, "Generation 1 threshold should be valid");
    TEST_ASSERT(threshold2 >= 0, "Generation 2 threshold should be valid");
    
    gc_return_code_t result = py_gc_set_threshold(0, 1000);
    TEST_ASSERT(result == GC_SUCCESS, "Setting generation 0 threshold should succeed");
    
    result = py_gc_set_threshold(1, 2000);
    TEST_ASSERT(result == GC_SUCCESS, "Setting generation 1 threshold should succeed");
    
    result = py_gc_set_threshold(2, 3000);
    TEST_ASSERT(result == GC_SUCCESS, "Setting generation 2 threshold should succeed");
    
    int32_t new_threshold0 = py_gc_get_threshold(0);
    int32_t new_threshold1 = py_gc_get_threshold(1);
    int32_t new_threshold2 = py_gc_get_threshold(2);
    
    TEST_ASSERT(new_threshold0 == 1000, "Generation 0 threshold should be updated");
    TEST_ASSERT(new_threshold1 == 2000, "Generation 1 threshold should be updated");
    TEST_ASSERT(new_threshold2 == 3000, "Generation 2 threshold should be updated");
    
    result = py_gc_set_threshold(3, 1000);
    TEST_ASSERT(result == GC_ERROR_INVALID_GENERATION, 
                "Setting threshold for invalid generation should fail");
    
    int32_t invalid_threshold = py_gc_get_threshold(3);
    TEST_ASSERT(invalid_threshold == -1, "Getting threshold for invalid generation should fail");
    
    py_gc_set_threshold(0, threshold0);
    py_gc_set_threshold(1, threshold1);
    py_gc_set_threshold(2, threshold2);
}

static void test_gc_error_handling(void) {
    printf("\n=== Testing Error Handling ===\n");
    
    gc_return_code_t result = py_gc_set_debug(0x01);
    TEST_ASSERT(result == GC_SUCCESS, "Setting debug flags should succeed");
    
    result = py_gc_clear_uncollectable();
    TEST_ASSERT(result == GC_SUCCESS, "Clearing uncollectable should succeed");
}

static void test_gc_cleanup(void) {
    printf("\n=== Testing GC Cleanup ===\n");
    
    gc_return_code_t result = py_gc_cleanup();
    TEST_ASSERT(result == GC_SUCCESS, "GC cleanup should succeed");
}

static void print_test_summary(void) {
    printf("\n=== Test Summary ===\n");
    printf("Total tests: %d\n", test_results.total_tests);
    printf("Passed: %d\n", test_results.passed_tests);
    printf("Failed: %d\n", test_results.failed_tests);
    printf("Success rate: %.1f%%\n", 
           (float)test_results.passed_tests / test_results.total_tests * 100);
    
    if (test_results.failed_tests == 0) {
        printf("\n✓ All tests passed successfully!\n");
    } else {
        printf("\n✗ %d test(s) failed. Please review the output above.\n", 
               test_results.failed_tests);
    }
}

int main(void) {
    printf("Python GC Rust FFI Integration Test Suite\n");
    printf("=========================================\n");
    
    test_gc_initialization();
    test_gc_enable_disable();
    test_gc_object_tracking();
    test_gc_collection();
    test_gc_statistics();
    test_gc_thresholds();
    test_gc_error_handling();
    test_gc_cleanup();
    
    print_test_summary();
    
    return (test_results.failed_tests == 0) ? 0 : 1;
} 